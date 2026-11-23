//! The host ends of an IP35 board's serial connections: the L1 and the I/O
//! chip's ports, each on a TCP port or, for the console under CI, in the
//! process.

use crate::config::MachineConfig;
use crate::dev::serial_host::{open_log, serial_line_with_override, CiSerialBackend, SerialBackend, SerialLine};
use std::sync::Arc;

/// One of the board's serial connections: what it is called, the TCP port
/// and the setting that names the port, the CI backend it uses when it is
/// the console, the log it keeps when it is not, a Unix socket with the setting that names it,
/// and where the host end goes.
pub(crate) struct SerialPort {
    pub label: &'static str,
    pub port: u16,
    pub setting: &'static str,
    pub ci: Arc<CiSerialBackend>,
    pub log: Option<String>,
    pub socket: Option<(String, &'static str)>,
    pub connect: Box<dyn Fn(Arc<dyn SerialBackend>) + Send + Sync>,
}

/// Connect the board's serial ports to the host: the console takes `serial_log` and, under CI, the
/// in-process backend. A taken TCP port leaves its stream unconnected; port 0 takes any free port.
pub(crate) fn connect_serial_ports(cfg: &MachineConfig, name: &str, ports: &[SerialPort], console: usize) -> Result<(), String> {
    for (i, a) in ports.iter().enumerate() {
        if let Some(b) = ports[i + 1..].iter().find(|b| b.port == a.port && a.port != 0) {
            return Err(format!("{name} {} and {} serial ports must be distinct", a.label, b.label));
        }
    }
    let mut log_config = cfg.clone();
    for (index, port) in ports.iter().enumerate() {
        log_config.serial_log = if index == console { cfg.serial_log.clone() } else { port.log.clone() };
        let tag = format!("{name} {}", port.label);
        let console_mark = if index == console { " (console)" } else { "" };
        let log = open_log(&log_config, &tag);
        let host = match port.setting {
            "serial_port_a" => cfg.serial_backends.a.as_ref(),
            "serial_port_b" => cfg.serial_backends.b.as_ref(),
            "tezro.serial_port_c" => cfg.serial_backends.c.as_ref(),
            "tezro.serial_port_d" => cfg.serial_backends.d.as_ref(),
            key if key.ends_with(".l1_port") => cfg.serial_backends.l1.as_ref(),
            _ => None,
        };
        let line = SerialLine {
            label: &format!("{tag}{console_mark}"),
            tcp_port: Some(port.port),
            tcp_setting: port.setting,
            socket: port.socket.as_ref().map(|(path, _)| path.as_str()),
            socket_setting: port.socket.as_ref().map_or("", |(_, setting)| setting),
        };
        let (backend, _) = serial_line_with_override(&line, log, (cfg.ci && index == console).then_some(&port.ci), host);
        (port.connect)(backend);
    }
    Ok(())
}


#[cfg(test)]
mod frontend_tests {
    use super::*;
    use parking_lot::Mutex;

    #[test]
    fn platform_transports_replace_tcp_and_keep_ci_fanout() {
        let occupied = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = occupied.local_addr().unwrap().port();
        for (setting, index) in [("fuel.l1_port", 0), ("serial_port_a", 1),
            ("serial_port_b", 2), ("tezro.serial_port_c", 3), ("tezro.serial_port_d", 4)] {
            let host = Arc::new(CiSerialBackend::new());
            let ci = Arc::new(CiSerialBackend::new());
            let mut cfg = MachineConfig::default();
            cfg.ci = true;
            let slot = match index {
                0 => &mut cfg.serial_backends.l1,
                1 => &mut cfg.serial_backends.a,
                2 => &mut cfg.serial_backends.b,
                3 => &mut cfg.serial_backends.c,
                _ => &mut cfg.serial_backends.d,
            };
            *slot = Some(host.clone());
            let installed = Arc::new(Mutex::new(None));
            let output = installed.clone();
            let ports = [SerialPort { label: "fixture", port, setting, ci: ci.clone(),
                log: None, socket: None, connect: Box::new(move |backend| *output.lock() = Some(backend)) }];
            connect_serial_ports(&cfg, "test", &ports, 0).unwrap();
            let backend = installed.lock().clone().unwrap();
            backend.send_byte(b'x');
            assert_eq!(host.drain_guest(), b"x");
            assert_eq!(ci.drain_guest(), b"x");
            host.push_host(b"h");
            assert_eq!(backend.recv_byte().unwrap(), b'h');
            ci.push_host(b"c");
            assert_eq!(backend.recv_byte().unwrap(), b'c');
        }
    }
}
