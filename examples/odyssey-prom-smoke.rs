//! Diskless bundled-PROM check without host sockets or a window.
//! Usage: cargo run --example odyssey-prom-smoke -- v12 /tmp/new-v12-run [seconds] [r10000|r12000|r14000] [cpus] [graphics|serial]
//! Graphics mode saves prom.png for visual inspection. Serial mode enters the
//! command monitor and checks hinv for CPU model, clock, count and cache size.
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
    let serial_console = match args.get(6).map(String::as_str).unwrap_or("graphics") {
        "graphics" => false,
        "serial" => true,
        _ => return Err("expected graphics or serial console".into()),
    };
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
    cfg.machine.cpu = match args.get(4).map(String::as_str).unwrap_or("r10000") {
        "r10000" => CpuModel::R10000,
        "r12000" => CpuModel::R12000,
        "r14000" => CpuModel::R14000,
        _ => return Err("expected r10000, r12000 or r14000".into()),
    };
    cfg.machine.cpus = args
        .get(5)
        .map(|s| s.parse())
        .transpose()
        .map_err(|_| "invalid CPU count")?
        .unwrap_or(1);
    println!(
        "{} x {}: Count {} Hz",
        cfg.machine.cpus,
        cfg.machine.cpu.label(),
        cfg.machine.cpu.octane_count_hz().unwrap()
    );
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
    cfg.no_audio = true;
    cfg.validate()?;
    let console = if serial_console {
        iris::prom_env::Console::Serial
    } else {
        iris::prom_env::Console::Graphics
    };
    iris::prom_env::prepare_environment(&cfg, None, Some(console))?;
    iris::workstation::set_exit_on_guest_poweroff(false);
    let mut machine = OctaneMachine::new(&cfg)?;
    let input = HostInput::attach(&machine);
    // Start peripherals and CPU directly: this probe needs no TCP monitor.
    machine.restart_peripherals();
    machine.cpu_start();
    let started = Instant::now();
    let mut transcript = Vec::new();
    let (mut continued, mut menu, mut inventory) = (false, false, None);
    let mut inventory_complete = false;
    while started.elapsed() < Duration::from_secs(seconds) {
        std::thread::sleep(Duration::from_millis(100));
        if serial_console {
            transcript.extend(a.drain_guest());
            let text = String::from_utf8_lossy(&transcript);
            if !continued && text.contains("Press any key") {
                a.push_host(b"\r");
                continued = true;
            }
            if !menu && (text.contains("Enter your choice") || text.contains("Option?")) {
                a.push_host(b"5\r"); // PROM command monitor.
                menu = true;
            }
            if inventory.is_none() && text.contains(">> ") {
                inventory = Some(transcript.len());
                a.push_host(b"hinv -v\r");
            } else if let Some(at) = inventory {
                let reply = String::from_utf8_lossy(&transcript[at..]);
                if reply.contains(">> ") {
                    let (mhz, cache_mb) = match cfg.machine.cpu {
                        CpuModel::R10000 => (195, 1),
                        CpuModel::R12000 => (300, 2),
                        CpuModel::R14000 => (600, 2),
                        _ => unreachable!(),
                    };
                    let expected_cpu = format!(
                        "{mhz} Mhz {},",
                        cfg.machine.cpu.label().trim_start_matches("MIPS ")
                    );
                    // PROM hinv prints the cache on its own line; it does
                    // not use IRIX's combined processor/cache inventory line.
                    let processors = reply.lines().filter(|line| {
                        line.split_once(':').is_some_and(|(name, value)| {
                            name.trim().starts_with("Processor")
                                && value.trim().starts_with(&expected_cpu)
                        })
                    }).count();
                    inventory_complete = processors == cfg.machine.cpus as usize
                        && reply.contains(&format!("Secondary cache size: {cache_mb} Mbytes"));
                    break;
                }
            }
        }
    }
    machine.get_cpu_debug().stop();
    let capture = (|| {
        if serial_console {
            if !inventory_complete {
                return Err("PROM inventory did not confirm CPU model, clock, count and cache before timeout".into());
            }
            println!("{}: PROM hinv identified the CPU", cfg.machine.cpu.label());
            return Ok(());
        }
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
    transcript.extend(a.drain_guest());
    std::fs::write(dir.join("serial-a.log"), transcript).map_err(|e| e.to_string())?;
    std::fs::write(dir.join("serial-b.log"), b.drain_guest()).map_err(|e| e.to_string())?;
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
