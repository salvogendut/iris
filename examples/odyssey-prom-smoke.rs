//! Diskless bundled-PROM check without host sockets or a window.
//! Usage: cargo run --example odyssey-prom-smoke -- v12 /tmp/new-v12-run [seconds]
//! Inspect prom.png: nonuniform pixels alone do not prove a successful boot.
use iris::config::{CpuModel, GraphicsBoard, MachineConfig, MachineProfile, NetMode};
use iris::dev::serial_host::CiSerialBackend;
use iris::hostinput::HostInput;
use iris::ip30_machine::OctaneMachine;
use iris::traits::Device;
use iris::workstation::Workstation;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

fn run() -> Result<(), String> {
    let args: Vec<_> = std::env::args().collect();
    let product = args.get(1).ok_or("expected v6, v8, v10 or v12")?;
    let board: GraphicsBoard = product.parse()?;
    if !board.is_vpro() {
        return Err("expected a VPro card".into());
    }
    let dir = PathBuf::from(args.get(2).ok_or("expected a new output directory")?);
    let seconds: u64 = args
        .get(3)
        .map(|s| s.parse())
        .transpose()
        .map_err(|_| "invalid duration")?
        .unwrap_or(45);
    if !(1..=120).contains(&seconds) {
        return Err("duration must be 1..120 seconds".into());
    }
    std::fs::create_dir(&dir).map_err(|e| format!("new output directory: {e}"))?;
    let a = Arc::new(CiSerialBackend::new());
    let b = Arc::new(CiSerialBackend::new());
    let mut cfg = MachineConfig::default();
    cfg.machine.profile = MachineProfile::OctaneIp30;
    cfg.machine.cpu = CpuModel::R10000;
    cfg.machine.cpus = 1;
    cfg.banks = [128, 0, 0, 0, 0, 0, 0, 0];
    cfg.graphics.board = Some(board);
    cfg.scsi.clear();
    cfg.network.mode = NetMode::Off;
    cfg.prom.clear(); // Bundled IP30 PROM 4.9.
    cfg.nvram = dir.join("nvram.bin").to_string_lossy().into_owned();
    cfg.octane.flash = Some(dir.join("flash.bin").to_string_lossy().into_owned());
    cfg.serial_backends.a = Some(a.clone());
    cfg.serial_backends.b = Some(b.clone());
    cfg.ci = true;
    cfg.validate()?;
    iris::prom_env::prepare_environment(&cfg, None, Some(iris::prom_env::Console::Graphics))?;
    iris::workstation::set_exit_on_guest_poweroff(false);
    let mut machine = OctaneMachine::new(&cfg)?;
    let input = HostInput::attach(&machine);
    // Start peripherals and CPU directly: this probe needs no TCP monitor.
    machine.restart_peripherals();
    machine.cpu_start();
    let started = Instant::now();
    while started.elapsed() < Duration::from_secs(seconds) {
        std::thread::sleep(Duration::from_secs(1));
    }
    machine.get_cpu_debug().stop();
    let capture = (|| {
        let (width, height, pixels) = input.frame()?;
        let first = pixels.first().copied().unwrap_or(0);
        let changed = pixels.iter().filter(|&&p| p != first).count();
        println!("{product}: {width}x{height}; {changed} pixels differ from the first pixel");
        let path = dir.join("prom.png");
        input.execute_command(
            "screen",
            &["shot", path.to_str().ok_or("non-UTF8 path")?],
            Box::new(std::io::stdout()),
        )?;
        if width != 1280 || height != 1024 || changed < 1000 {
            return Err("expected a nonuniform 1280x1024 PROM display".into());
        }
        Ok(())
    })();
    machine.stop();
    for (name, serial) in [("serial-a.log", a), ("serial-b.log", b)] {
        std::fs::write(dir.join(name), serial.drain_guest()).map_err(|e| e.to_string())?;
    }
    capture
}

fn main() {
    let result = std::thread::Builder::new()
        .name("odyssey-prom-smoke".into())
        .stack_size(64 * 1024 * 1024)
        .spawn(run)
        .expect("probe thread")
        .join()
        .expect("probe panic");
    if let Err(error) = result {
        eprintln!("Odyssey PROM smoke: {error}");
        std::process::exit(1);
    }
}
