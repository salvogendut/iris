//! An adapter's connection to its network backend, made from `[network]`:
//! the two frame rings, their wake-ups and the backend at the far end. The
//! adapter takes one through `attach` and knows nothing of NAT or PCAP;
//! a new backend is one `NetBackend` implementation and a line in `Link::backend`.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use parking_lot::Mutex;

use crate::config::{NetMode, NetworkConfig};

use super::{GatewayConfig, NatControl, NatEngine, NetBackend, Wake};

/// The adapter's ends of a link.
pub struct Rings {
    /// Frames the guest sends.
    pub to_net: rtrb::Producer<Vec<u8>>,
    /// Frames for the guest.
    pub from_net: rtrb::Consumer<Vec<u8>>,
    /// Told when a frame is on `to_net`.
    pub tx_wake: Arc<Wake>,
    /// Told by the backend when a frame is on `from_net`; the adapter notifies
    /// it too when the guest does something that can unblock a waiting frame.
    pub rx_wake: Arc<Wake>,
}

pub struct Link {
    rings: Rings,
    /// The backend's ends of the rings, until the backend is built.
    far: Option<(rtrb::Consumer<Vec<u8>>, rtrb::Producer<Vec<u8>>)>,
    /// Guest frames processed, whichever the backend: the GUI's indicator.
    frames: Arc<AtomicU64>,
    /// The backend's controls and status (the NAT's counters and tables, the
    /// PCAP bridge's state); renewed links share them.
    nat: Arc<NatControl>,
    /// Cleared to stop the backend.
    running: Arc<AtomicBool>,
    config: GatewayConfig,
    worker: Arc<Mutex<Option<BackendWorker>>>,
}

struct BackendWorker {
    thread: Option<std::thread::JoinHandle<()>>,
    running: Arc<AtomicBool>,
    wake: Arc<Wake>,
}

impl BackendWorker {
    fn stop(mut self) {
        self.running.store(false, Ordering::SeqCst);
        self.wake.notify();
        if let Some(thread) = self.thread.take() { let _ = thread.join(); }
    }
}

impl Link {
    /// `None` for `mode = "off"`: an adapter left unplugged.
    pub fn open(net: &NetworkConfig) -> Result<Option<Link>, String> {
        Self::with_gateway(GatewayConfig::from_network(net))
    }

    /// As `open`, for a gateway of the caller's own (a DaynaPort's subnet).
    pub fn with_gateway(config: GatewayConfig) -> Result<Option<Link>, String> {
        if config.mode == NetMode::Off {
            return Ok(None);
        }
        let frames = Arc::new(AtomicU64::new(0));
        Ok(Some(Self::build(config, frames.clone(), NatControl::counting(frames), Wake::new(), Wake::new())))
    }

    /// A link to the same backend with fresh rings and a backend that has not
    /// been built, sharing the wake-ups and controls: what a stopped adapter
    /// runs on its next start.
    pub fn renew(&self) -> Result<Link, String> {
        let r = &self.rings;
        let mut next = Self::build(self.config.clone(), self.frames.clone(), self.nat.clone(), r.tx_wake.clone(), r.rx_wake.clone());
        next.worker = self.worker.clone();
        Ok(next)
    }

    fn build(config: GatewayConfig, frames: Arc<AtomicU64>, nat: Arc<NatControl>, tx_wake: Arc<Wake>, rx_wake: Arc<Wake>) -> Link {
        let (to_net, tx_cons) = rtrb::RingBuffer::new(256);
        let (rx_prod, from_net) = rtrb::RingBuffer::new(256);
        let rings = Rings { to_net, from_net, tx_wake, rx_wake };
        Link { rings, far: Some((tx_cons, rx_prod)), frames, nat, running: Arc::new(AtomicBool::new(false)), config,
               worker: Arc::new(Mutex::new(None)) }
    }

    /// Which backend, for the adapter's log line.
    pub fn describe(&self) -> String {
        match self.config.mode {
            NetMode::Pcap if cfg!(feature = "pcap") => "PCAP (bridged)".into(),
            NetMode::Pcap => "NAT (software gateway): mode = \"pcap\" needs a build with --features pcap".into(),
            _ => "NAT (software gateway)".into(),
        }
    }

    pub fn frames(&self) -> Arc<AtomicU64> {
        self.frames.clone()
    }

    pub fn nat(&self) -> Arc<NatControl> {
        self.nat.clone()
    }

    /// The flag the backend watches; clear it to stop the backend.
    pub fn running(&self) -> Arc<AtomicBool> {
        self.running.clone()
    }

    /// The wake-up the backend notifies when it has a frame for the guest.
    pub fn rx_wake(&self) -> Arc<Wake> {
        self.rings.rx_wake.clone()
    }

    /// The adapter's ends of the rings, once the backend has its own.
    pub fn into_rings(self) -> Rings {
        self.rings
    }

    /// The backend `config.mode` names: the NAT gateway or a PCAP bridge,
    /// which needs `--features pcap` (without it the NAT runs instead). It is
    /// built here, not when the link is made: the NAT binds its host listeners
    /// when built. `None` if it was built already.
    pub fn backend(&mut self) -> Option<Box<dyn NetBackend>> {
        let (tx_cons, rx_prod) = self.far.take()?;
        let config = self.nat.runtime_config(self.config.clone());
        let (rx_wake, tx_wake) = (self.rings.rx_wake.clone(), self.rings.tx_wake.clone());
        let (running, ctl) = (self.running.clone(), self.nat.clone());
        #[cfg(feature = "pcap")]
        if config.mode == NetMode::Pcap {
            return Some(Box::new(super::net_pcap::PcapEngine::new(config, tx_cons, rx_prod, rx_wake, tx_wake, running, ctl)));
        }
        Some(Box::new(NatEngine::new(config, tx_cons, rx_prod, rx_wake, tx_wake, running, ctl)))
    }

    /// Run the backend on a thread called `name` until `running` clears. The
    /// stopped adapter need not join here: the next start waits for its old
    /// backend to release sockets before binding replacements. An adapter
    /// which consumes a link without renewing it still owns the running flag;
    /// dropping the link alone does not stop that detached backend.
    pub fn spawn(&mut self, name: &str) {
        if self.far.is_none() { return; }
        let shared = self.worker.clone();
        let mut worker = shared.lock();
        if let Some(previous) = worker.take() { previous.stop(); }
        let Some(mut backend) = self.backend() else { return };
        self.running.store(true, Ordering::SeqCst);
        let thread = std::thread::Builder::new().name(name.into()).spawn(move || backend.run()).expect("network backend thread");
        *worker = Some(BackendWorker { thread: Some(thread), running: self.running.clone(), wake: self.rings.tx_wake.clone() });
    }

    /// Rings to nowhere, and their far ends, for adapter tests.
    #[cfg(test)]
    pub fn loopback() -> (Link, rtrb::Consumer<Vec<u8>>, rtrb::Producer<Vec<u8>>) {
        let mut link = Self::build(GatewayConfig::default(), Arc::default(), NatControl::new(), Wake::new(), Wake::new());
        let (tx_cons, rx_prod) = link.far.take().expect("a new link has its far ends");
        (link, tx_cons, rx_prod)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renewed_links_retain_consumed_live_settings() {
        use crate::config::{ForwardBind, ForwardProto, PortForwardConfig};
        let first = Link::open(&NetworkConfig::default()).unwrap().unwrap();
        let next = first.renew().unwrap(); // Already prepared before the edit.
        let ctl = first.nat();
        ctl.request_subnet("10.44.0.1".parse().unwrap(), "10.44.0.2".parse().unwrap(), "255.255.255.0".parse().unwrap());
        ctl.set_port_forwards(vec![PortForwardConfig { proto: ForwardProto::Tcp, host_port: 23456, guest_port: 23, bind: ForwardBind::Localhost }]);
        ctl.request_pcap_interface(Some("test-interface".into()));
        // The previous backend has applied the notifications already.
        ctl.apply_subnet.store(false, Ordering::Release);
        ctl.apply_forwards.store(false, Ordering::Release);
        ctl.apply_pcap_iface.store(false, Ordering::Release);
        ctl.pending_forwards.lock().take();
        let config = next.nat.runtime_config(next.config.clone());
        assert_eq!(config.gateway_ip.to_string(), "10.44.0.1");
        assert_eq!(config.client_ip.to_string(), "10.44.0.2");
        assert_eq!(config.port_forwards.len(), 1);
        assert_eq!(config.port_forwards[0].host_port, 23456);
        assert_eq!(config.pcap_interface.as_deref(), Some("test-interface"));
        ctl.set_port_forwards(vec![]);
        ctl.request_pcap_interface(None);
        let config = next.nat.runtime_config(config);
        assert!(config.port_forwards.is_empty());
        assert!(config.pcap_interface.is_none());
    }

    #[test]
    fn rapid_restart_releases_old_forwarding_ports_before_binding() {
        use std::net::{Ipv4Addr, TcpListener, TcpStream};
        use crate::config::{ForwardBind, ForwardProto, PortForwardConfig};
        let reserve = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = reserve.local_addr().unwrap().port();
        drop(reserve);
        let cfg = GatewayConfig { port_forwards: vec![PortForwardConfig {
            proto: ForwardProto::Tcp, host_port: port, guest_port: 23, bind: ForwardBind::Localhost,
        }], ..Default::default() };
        let mut first = Link::with_gateway(cfg).unwrap().unwrap();
        let mut next = first.renew().unwrap();
        first.spawn("test-net-old");
        // Do not wait for the old backend to observe its stop flag.
        first.running.store(false, Ordering::SeqCst);
        next.spawn("test-net-new");
        assert!(TcpStream::connect((Ipv4Addr::LOCALHOST, port)).is_ok());
        next.worker.lock().take().unwrap().stop();
        assert!(TcpStream::connect((Ipv4Addr::LOCALHOST, port)).is_err());
    }

    /// A renewed link has fresh rings and a backend that has not been built,
    /// and shares the wake-ups and controls.
    #[test]
    fn a_renewed_link_is_fresh() {
        let first = Link::open(&NetworkConfig::default()).unwrap().unwrap();
        let next = first.renew().unwrap();
        assert!(Arc::ptr_eq(&first.nat, &next.nat) && Arc::ptr_eq(&first.rings.tx_wake, &next.rings.tx_wake));
        assert!(next.far.is_some() && !Arc::ptr_eq(&first.running, &next.running));
    }

    #[test]
    fn a_backend_is_built_once() {
        let mut link = Link::open(&NetworkConfig::default()).unwrap().unwrap();
        assert!(link.backend().is_some());
        assert!(link.backend().is_none());
    }

    #[test]
    fn an_unplugged_network_makes_no_link() {
        let net = NetworkConfig { mode: NetMode::Off, ..NetworkConfig::default() };
        assert!(Link::open(&net).unwrap().is_none());
    }
}
