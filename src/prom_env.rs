//! Host-selected boot and console variables in the PROM's persistent environment.
//!
//! Layouts: IP22/IP28 fixed NVRAM fields, IP32 SHDR environment segments,
//! and IP30 FPDS environment records. Firmware code and unrelated variables
//! remain intact. Call only while the machine is unloaded and its state is locked.

use crate::config::{MachineConfig, MachineProfile};
use std::{fs, io::ErrorKind, path::Path};

const NAMES: [&[u8]; 4] = [b"OSLoader", b"OSLoadFilename", b"OSLoadOptions", b"console"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Console {
    Graphics,
    Serial,
}

impl Console {
    pub fn value(self) -> &'static str {
        match self {
            Self::Graphics => "g",
            Self::Serial => "d",
        }
    }
}

/// Apply three boot variables before constructing a workstation. This performs
/// file I/O and must not run on a UI thread or alongside a loaded machine.
/// Each replacement is atomic; malformed/unknown images are left untouched.
/// `AutoLoad`, partition choices, MAC and the remaining environment are preserved.
pub fn prepare_boot_environment(
    cfg: &MachineConfig,
    loader: &str,
    filename: &str,
    options: &str,
) -> Result<(), String> {
    prepare_environment(cfg, Some([loader, filename, options]), None)
}

/// Apply independently selected boot and console presets before machine creation.
/// `None` leaves that selection untouched. The caller must hold the state lease.
/// All images are validated before writing. Replacements are atomic per file;
/// an I/O failure on a later file attempts to restore earlier files.
pub fn prepare_environment(
    cfg: &MachineConfig,
    boot: Option<[&str; 3]>,
    console: Option<Console>,
) -> Result<(), String> {
    if boot.is_none() && console.is_none() {
        return Ok(());
    }
    for (value, limit) in boot.into_iter().flatten().zip([18, 28, 12]) {
        if value.is_empty() || value.len() >= limit || !value.bytes().all(|b| b.is_ascii_graphic())
        {
            return Err(
                "PROM boot values must be nonempty printable ASCII and fit the firmware fields"
                    .into(),
            );
        }
    }
    let values = [
        boot.map(|v| v[0]),
        boot.map(|v| v[1]),
        boot.map(|v| v[2]),
        console.map(Console::value),
    ];
    let mut pending = Vec::new();
    if cfg.machine.profile == MachineProfile::OctaneIp30 {
        if let Some(console) = console {
            let mut state = PendingState::read(&cfg.nvram, || Ok(vec![0; 512]))?;
            octane_console(&mut state.bytes, console)?;
            pending.push(state);
        }
        if boot.is_none() {
            return save_states(&pending);
        }
    }
    let path = match cfg.machine.profile {
        MachineProfile::Indigo2Ip22 | MachineProfile::Indigo2Ip28 => &cfg.nveeprom,
        MachineProfile::OctaneIp30 => cfg
            .octane
            .flash
            .as_ref()
            .ok_or("PROM boot settings require an Octane flash file")?,
        _ => &cfg.nvram,
    };
    let mut state = PendingState::read(path, || {
        Ok(match cfg.machine.profile {
            MachineProfile::IndyIp24 => vec![0; 8192],
            MachineProfile::Indigo2Ip22 | MachineProfile::Indigo2Ip28 => vec![0xff; 256],
            MachineProfile::O2Ip32 => crate::firmware::load_flash_prom(
                &cfg.prom,
                &[
                    Path::new(crate::config::DEFAULT_PROM),
                    Path::new(crate::ip32_machine::O2_PROM_FILE),
                ],
                &crate::prombinip32::PROM_IP32_REV4_18,
                "O2",
            )?,
            MachineProfile::OctaneIp30 => {
                let mut image = crate::firmware::load_flash_prom(
                    &cfg.prom,
                    &[
                        Path::new(crate::config::DEFAULT_PROM),
                        Path::new(crate::ip30_machine::OCTANE_PROM_FILE),
                    ],
                    &crate::prombinip30::PROM_IP30_REV4_9,
                    "Octane",
                )?;
                if image.len() > 1 << 20 {
                    return Err("Octane PROM exceeds flash size".into());
                }
                image.resize(1 << 20, 0xff);
                image
            }
        })
    })?;
    let bytes = &mut state.bytes;
    match cfg.machine.profile {
        MachineProfile::IndyIp24 | MachineProfile::Indigo2Ip22 | MachineProfile::Indigo2Ip28 => {
            fixed_environment(bytes, cfg, values)?;
        }
        MachineProfile::O2Ip32 => o2_environment(bytes, values)?,
        MachineProfile::OctaneIp30 => {
            octane_environment(bytes, [values[0], values[1], values[2], None])?
        }
    }
    pending.push(state);
    save_states(&pending)
}

struct PendingState<'a> {
    path: &'a str,
    original: Option<Vec<u8>>,
    bytes: Vec<u8>,
}

impl<'a> PendingState<'a> {
    fn read(path: &'a str, seed: impl FnOnce() -> Result<Vec<u8>, String>) -> Result<Self, String> {
        if path.is_empty() {
            return Err("PROM settings require a persistent firmware state path".into());
        }
        let original = match fs::read(path) {
            Ok(bytes) => Some(bytes),
            Err(e) if e.kind() == ErrorKind::NotFound => None,
            Err(e) => return Err(format!("Cannot read PROM state {path}: {e}")),
        };
        let bytes = match &original {
            Some(bytes) => bytes.clone(),
            None => seed()?,
        };
        Ok(Self {
            path,
            original,
            bytes,
        })
    }
}

fn save_states(states: &[PendingState<'_>]) -> Result<(), String> {
    for (i, state) in states.iter().enumerate() {
        if state.original.as_ref() == Some(&state.bytes) {
            continue;
        }
        if let Err(e) = crate::nv_storage::save(state.path, &state.bytes) {
            let mut error = format!("Cannot save PROM state {}: {e}", state.path);
            for previous in states[..i].iter().rev() {
                if previous.original.as_ref() == Some(&previous.bytes) {
                    continue;
                }
                let restored = match &previous.original {
                    Some(bytes) => crate::nv_storage::save(previous.path, bytes),
                    None => fs::remove_file(previous.path),
                };
                if let Err(e) = restored {
                    error.push_str(&format!("; cannot restore {}: {e}", previous.path));
                }
            }
            return Err(error);
        }
    }
    Ok(())
}

fn nv_checksum(bytes: &[u8]) -> u8 {
    bytes
        .chunks_exact(2)
        .enumerate()
        .fold(0xa5u8, |sum, (i, pair)| {
            (sum ^ pair[1] ^ if i == 0 { 0 } else { pair[0] }).rotate_left(1)
        })
}

fn fixed_environment(
    bytes: &mut [u8],
    cfg: &MachineConfig,
    values: [Option<&str>; 4],
) -> Result<(), String> {
    let indy = cfg.machine.profile == MachineProfile::IndyIp24;
    let expected = if indy { 8192 } else { 256 };
    if bytes.len() != expected {
        return Err(format!(
            "PROM state is {} bytes; expected {expected}",
            bytes.len()
        ));
    }
    let offset = if indy { 64 } else { 0 };
    let nv = &mut bytes[offset..offset + 256];
    let revision = if cfg.machine.profile == MachineProfile::Indigo2Ip28 {
        9
    } else {
        8
    };
    let blank = nv[..250].iter().all(|&b| b == 0) || nv[..250].iter().all(|&b| b == 0xff);
    if blank {
        nv[..250].fill(0);
        nv[1] = revision;
        // Defaults for a new environment; existing environments never take this path.
        for (at, value) in [
            (2, "g"),
            (116, "9600"),
            (121, "0"),
            (122, "PST8PDT"),
            (178, "Y"),
            (232, "80"),
            (235, "0"),
            (236, "y"),
            (239, "y"),
        ] {
            nv[at..at + value.len()].copy_from_slice(value.as_bytes());
        }
        if revision == 9 {
            nv[242] = b'1';
        }
        if nv[250..].iter().all(|&b| b == 0) || nv[250..].iter().all(|&b| b == 0xff) {
            nv[250..].copy_from_slice(&cfg.network().mac);
        }
    } else if nv[1] != revision || nv[0] != nv_checksum(nv) {
        return Err("PROM NVRAM has an unsupported revision or invalid checksum; boot with Keep existing to initialize it first".into());
    }
    // Match the constructor's MAC seeding before computing the checksum, so
    // a blank address cannot invalidate an otherwise valid preset on startup.
    let erased_mac = if indy { 0 } else { 0xff };
    if nv[250..].iter().all(|&b| b == erased_mac) {
        nv[250..].copy_from_slice(&cfg.network().mac);
    }
    for ((at, size), value) in [(52, 18), (70, 28), (98, 12), (2, 2)]
        .into_iter()
        .zip(values)
    {
        let Some(value) = value else { continue };
        nv[at..at + size].fill(0);
        nv[at..at + value.len()].copy_from_slice(value.as_bytes());
    }
    nv[0] = nv_checksum(nv);
    Ok(())
}

fn sum32(bytes: &[u8]) -> u32 {
    bytes.chunks_exact(4).fold(0u32, |sum, word| {
        sum.wrapping_add(u32::from_be_bytes(word.try_into().unwrap()))
    })
}

fn variable(record: &[u8]) -> Option<(usize, &[u8])> {
    let end = record.iter().position(|&b| b == 0)?;
    let record = &record[..end];
    let equal = record.iter().position(|&b| b == b'=')?;
    let index = NAMES
        .iter()
        .position(|name| record[..equal].eq_ignore_ascii_case(name))?;
    Some((index, &record[equal + 1..]))
}

fn o2_environment(bytes: &mut [u8], values: [Option<&str>; 4]) -> Result<(), String> {
    if bytes.len() != 512 * 1024 {
        return Err("O2 flash must be 512 KiB".into());
    }
    let mut at = 0;
    while at + 64 <= bytes.len() {
        let header = &bytes[at..at + 64];
        if &header[8..12] != b"SHDR" || sum32(header) != 0 {
            at += 256;
            continue;
        }
        let size = u32::from_be_bytes(header[12..16].try_into().unwrap()) as usize;
        if size < 68 || size % 4 != 0 || size > bytes.len() - at {
            return Err("Invalid O2 flash segment length".into());
        }
        if header[16] != 3 || &header[20..23] != b"env" {
            at += size.next_multiple_of(256);
            continue;
        }
        if sum32(&bytes[at + 64..at + size]) != 0 {
            return Err("Invalid O2 PROM environment checksum".into());
        }
        let body = &bytes[at + 64..at + size - 4];
        let mut rewritten = Vec::with_capacity(body.len());
        let mut rest = body;
        loop {
            let end = rest
                .iter()
                .position(|&b| b == 0)
                .ok_or("Unterminated O2 PROM environment")?;
            if end == 0 {
                break;
            }
            let record = &rest[..=end];
            if !record.contains(&b'=') {
                return Err("Malformed O2 PROM environment record".into());
            }
            if !variable(record).is_some_and(|(i, _)| values[i].is_some()) {
                rewritten.extend_from_slice(record);
            }
            rest = &rest[end + 1..];
        }
        for (name, value) in NAMES.into_iter().zip(values) {
            let Some(value) = value else { continue };
            rewritten.extend_from_slice(name);
            rewritten.push(b'=');
            rewritten.extend_from_slice(value.as_bytes());
            rewritten.push(0);
        }
        rewritten.push(0);
        if rewritten.len() > body.len() {
            return Err("O2 PROM environment is full".into());
        }
        rewritten.resize(body.len(), 0);
        let checksum = sum32(&rewritten).wrapping_neg();
        bytes[at + 64..at + size - 4].copy_from_slice(&rewritten);
        bytes[at + size - 4..at + size].copy_from_slice(&checksum.to_be_bytes());
        return Ok(());
    }
    Err(
        "O2 PROM has no valid environment segment; boot with Keep existing to initialize it first"
            .into(),
    )
}

fn sum16(bytes: &[u8]) -> u16 {
    let mut sum = bytes.chunks_exact(2).fold(0u32, |sum, word| {
        sum + u16::from_be_bytes(word.try_into().unwrap()) as u32
    });
    while sum > 0xffff {
        sum = (sum & 0xffff) + (sum >> 16);
    }
    sum as u16
}

fn octane_environment(bytes: &mut [u8], values: [Option<&str>; 4]) -> Result<(), String> {
    if bytes.len() != 1024 * 1024 {
        return Err("Octane flash must be 1 MiB".into());
    }
    let pds = &mut bytes[0xf0000..];
    if &pds[..4] != b"FPDS" || sum16(&pds[..24]) != 0xffff {
        return Err("Octane PROM has no valid persistent-data header; boot with Keep existing to initialize it first".into());
    }
    let mut found: [Vec<(usize, Vec<u8>)>; 4] = Default::default();
    let mut at = 24;
    while at + 4 <= pds.len() {
        let kind = u16::from_be_bytes(pds[at..at + 2].try_into().unwrap());
        if kind == 0xffff {
            break;
        }
        let size = 4 + (kind & 0xff) as usize * 2;
        if size == 4 || size > pds.len() - at {
            return Err("Invalid Octane PROM environment record length".into());
        }
        if kind & 0xff00 == 0x0100 && &pds[at + 2..at + 4] == [0xff, 0xff] {
            let data = &pds[at + 4..at + size];
            if !data.contains(&0) || !data.contains(&b'=') {
                return Err("Malformed Octane PROM environment record".into());
            }
            if let Some((index, value)) = variable(data) {
                found[index].push((at, value.to_vec()));
            }
        }
        at += size;
    }
    for (i, value) in values.into_iter().enumerate() {
        let Some(value) = value else { continue };
        if found[i].len() == 1 && found[i][0].1 == value.as_bytes() {
            continue;
        }
        let mut record = NAMES[i].to_vec();
        record.push(b'=');
        record.extend_from_slice(value.as_bytes());
        record.push(0);
        record.resize(record.len().next_multiple_of(2), 0);
        let end = at + 4 + record.len();
        if end + 4 > pds.len() || pds[at..end + 4].iter().any(|&b| b != 0xff) {
            return Err("Octane PROM persistent storage is full or not erased".into());
        }
        for (old, _) in &found[i] {
            pds[old + 2..old + 4].fill(0);
        }
        pds[at..at + 2].copy_from_slice(&(0x0100u16 | (record.len() / 2) as u16).to_be_bytes());
        pds[at + 4..end].copy_from_slice(&record);
        at = end;
    }
    Ok(())
}

// The Octane stores console in DS1685 RAM, independently of boot variables in
// flash. The logical 242-byte environment skips bank-1 registers in Iris's file.
fn octane_console(bytes: &mut [u8], console: Console) -> Result<(), String> {
    if bytes.len() != 512 {
        return Err("Octane PROM NVRAM must be 512 bytes".into());
    }
    let mut nv = [0u8; 242];
    nv[..114].copy_from_slice(&bytes[0x0e..0x80]);
    nv[114..].copy_from_slice(&bytes[0xc0..0x140]);
    if nv[1..].iter().all(|&b| b == 0) || nv[1..].iter().all(|&b| b == 0xff) {
        nv[1..].fill(0);
        nv[2] = 7;
        for (at, value) in [
            (11, "9600"),
            (19, "0"),
            (20, "Y"),
            (72, "80"),
            (75, "y"),
            (94, "1"),
            (103, "y"),
        ] {
            nv[at..at + value.len()].copy_from_slice(value.as_bytes());
        }
        nv[101..103].copy_from_slice(&0xb210u16.to_be_bytes());
    } else if nv[2] != 7 || nv[1] != octane_nv_checksum(&nv) {
        return Err("Octane PROM NVRAM has an unsupported revision or invalid checksum; boot with Keep existing to initialize it first".into());
    }
    nv[3] = console.value().as_bytes()[0];
    nv[4] = 0;
    nv[1] = octane_nv_checksum(&nv);
    bytes[0x0e..0x80].copy_from_slice(&nv[..114]);
    bytes[0xc0..0x140].copy_from_slice(&nv[114..]);
    Ok(())
}

fn octane_nv_checksum(nv: &[u8; 242]) -> u8 {
    nv[2..]
        .iter()
        .fold(0xa5u8, |sum, &b| (sum ^ b).rotate_left(1))
}

#[cfg(test)]
mod tests {
    use super::*;
    const NETBSD: [Option<&str>; 4] = [Some("boot"), Some("netbsd.boot"), Some("auto"), None];
    const IRIX: [Option<&str>; 4] = [Some("sash"), Some("/unix"), Some("auto"), None];

    #[test]
    fn console_updates_preserve_fixed_boot_fields_and_other_nvram() {
        for profile in [
            MachineProfile::IndyIp24,
            MachineProfile::Indigo2Ip22,
            MachineProfile::Indigo2Ip28,
        ] {
            let mut cfg = MachineConfig::default();
            cfg.machine.profile = profile;
            let base = if profile == MachineProfile::IndyIp24 {
                64
            } else {
                0
            };
            let mut image = vec![0; if base == 64 { 8192 } else { 256 }];
            fixed_environment(&mut image, &cfg, NETBSD).unwrap();
            let original = image.clone();
            fixed_environment(&mut image, &cfg, [None, None, None, Some("d")]).unwrap();
            assert_eq!(&image[base + 2..base + 4], b"d\0");
            assert_eq!(image[base], nv_checksum(&image[base..base + 256]));
            for (i, (&before, &after)) in original.iter().zip(&image).enumerate() {
                if i != base && !(base + 2..base + 4).contains(&i) {
                    assert_eq!(before, after, "{profile:?} byte {i}");
                }
            }
            fixed_environment(&mut image, &cfg, IRIX).unwrap();
            assert_eq!(&image[base + 2..base + 4], b"d\0");
            fixed_environment(&mut image, &cfg, [None, None, None, Some("g")]).unwrap();
            assert_eq!(&image[base + 2..base + 4], b"g\0");
        }
    }

    #[test]
    fn o2_console_and_boot_choices_are_independent() {
        let mut image = crate::prombinip32::PROM_IP32_REV4_18.to_vec();
        let mut values = NETBSD;
        values[3] = Some("d");
        o2_environment(&mut image, values).unwrap();
        o2_environment(&mut image, IRIX).unwrap();
        assert!(image.windows(10).any(|b| b == b"console=d\0"));
        o2_environment(&mut image, [None, None, None, Some("g")]).unwrap();
        assert!(image.windows(10).any(|b| b == b"console=g\0"));
        assert!(image.windows(21).any(|b| b == b"OSLoadFilename=/unix\0"));
        assert_eq!(sum32(&image[0x4040..0x4400]), 0);
        let once = image.clone();
        o2_environment(&mut image, [None, None, None, Some("g")]).unwrap();
        assert_eq!(once, image);
    }

    #[test]
    fn octane_console_preserves_clock_banks_and_extended_nvram() {
        let mut image = vec![0; 512];
        image[..0x0f].fill(0x22); // Clock registers and diagnostics scratch byte.
        image[0x80..0xc0].fill(0x33); // Bank-1 registers, not environment.
        image[0x140..].fill(0x44);
        octane_console(&mut image, Console::Graphics).unwrap();
        image[0xc0..0x140].fill(0x55);
        let nv: Vec<_> = image[0x10..0x80]
            .iter()
            .chain(&image[0xc0..0x140])
            .copied()
            .collect();
        image[0x0f] = nv.iter().fold(0xa5u8, |sum, &b| (sum ^ b).rotate_left(1));
        let original = image.clone();
        octane_console(&mut image, Console::Serial).unwrap();
        assert_eq!(&image[0x11..0x13], b"d\0");
        for (i, (&before, &after)) in original.iter().zip(&image).enumerate() {
            if i != 0x0f && !(0x11..0x13).contains(&i) {
                assert_eq!(before, after, "byte {i}");
            }
        }
        let once = image.clone();
        octane_console(&mut image, Console::Serial).unwrap();
        assert_eq!(image, once);
        octane_console(&mut image, Console::Graphics).unwrap();
        assert_eq!(image, original);
        image[0x0f] ^= 1;
        let bad = image.clone();
        assert!(octane_console(&mut image, Console::Serial).is_err());
        assert_eq!(image, bad);
    }

    #[test]
    fn later_save_failure_restores_existing_files_and_removes_new_files() {
        let dir = std::env::temp_dir().join(format!("iris-prom-rollback-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let first = dir.join("nvram.bin");
        let blocked = dir.join("flash.bin");
        fs::create_dir_all(&blocked).unwrap();
        for original in [Some(b"previous state".to_vec()), None] {
            if let Some(bytes) = &original {
                fs::write(&first, bytes).unwrap();
            }
            let states = [
                PendingState {
                    path: first.to_str().unwrap(),
                    original: original.clone(),
                    bytes: b"new state".to_vec(),
                },
                PendingState {
                    path: blocked.to_str().unwrap(),
                    original: None,
                    bytes: b"new flash".to_vec(),
                },
            ];
            assert!(
                save_states(&states)
                    .unwrap_err()
                    .contains("Cannot save PROM state")
            );
            assert_eq!(fs::read(&first).ok(), original);
            if first.exists() {
                fs::remove_file(&first).unwrap();
            }
        }
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn fixed_preserves_settings_and_checksums_across_presets() {
        for profile in [
            MachineProfile::IndyIp24,
            MachineProfile::Indigo2Ip22,
            MachineProfile::Indigo2Ip28,
        ] {
            let mut cfg = MachineConfig::default();
            cfg.machine.profile = profile;
            let indy = profile == MachineProfile::IndyIp24;
            let base = if indy { 64 } else { 0 };
            let mut image = vec![0; if indy { 8192 } else { 256 }];
            fixed_environment(&mut image, &cfg, IRIX).unwrap();
            image[base + 178] = b'N';
            image[base + 250..base + 256].copy_from_slice(&[8, 0, 0x69, 1, 2, 3]);
            image[base] = nv_checksum(&image[base..base + 256]);
            let original = image.clone();
            fixed_environment(&mut image, &cfg, NETBSD).unwrap();
            assert_eq!(&image[base + 52..base + 57], b"boot\0");
            assert_eq!(&image[base + 70..base + 82], b"netbsd.boot\0");
            assert_eq!(&image[base + 98..base + 103], b"auto\0");
            assert_eq!(image[base], nv_checksum(&image[base..base + 256]));
            for (i, (&before, &after)) in original.iter().zip(&image).enumerate() {
                if i != base && !(base + 52..base + 110).contains(&i) {
                    assert_eq!(before, after, "{profile:?} byte {i}");
                }
            }
            fixed_environment(&mut image, &cfg, IRIX).unwrap();
            assert_eq!(image, original);
            image[base] ^= 1;
            let bad = image.clone();
            assert!(fixed_environment(&mut image, &cfg, NETBSD).is_err());
            assert_eq!(image, bad);
        }
    }

    #[test]
    fn constructor_mac_seeding_cannot_invalidate_boot_variables() {
        for profile in [MachineProfile::IndyIp24, MachineProfile::Indigo2Ip22] {
            let mut cfg = MachineConfig::default();
            cfg.machine.profile = profile;
            let indy = profile == MachineProfile::IndyIp24;
            let base = if indy { 64 } else { 0 };
            let mut image = vec![0; if indy { 8192 } else { 256 }];
            fixed_environment(&mut image, &cfg, IRIX).unwrap();
            image[base + 250..base + 256].fill(if indy { 0 } else { 0xff });
            image[base] = nv_checksum(&image[base..base + 256]);
            fixed_environment(&mut image, &cfg, NETBSD).unwrap();
            assert_eq!(&image[base + 250..base + 256], &cfg.network().mac);
            assert_eq!(image[base], nv_checksum(&image[base..base + 256]));
        }
    }

    #[test]
    fn o2_updates_only_environment_body_and_preserves_unknown_variables() {
        let mut image = crate::prombinip32::PROM_IP32_REV4_18.to_vec();
        let original = image.clone();
        o2_environment(&mut image, NETBSD).unwrap();
        assert_eq!(&image[..0x4040], &original[..0x4040]);
        assert_eq!(&image[0x4400..], &original[0x4400..]);
        assert_eq!(sum32(&image[0x4040..0x4400]), 0);
        for value in [
            b"OSLoader=boot\0".as_slice(),
            b"OSLoadFilename=netbsd.boot\0",
            b"OSLoadOptions=auto\0",
            b"AutoLoad=Yes\0",
            b"console=g\0",
        ] {
            assert!(
                image[0x4040..0x4400]
                    .windows(value.len())
                    .any(|bytes| bytes == value)
            );
        }
        let once = image.clone();
        o2_environment(&mut image, NETBSD).unwrap();
        assert_eq!(image, once);
        o2_environment(&mut image, IRIX).unwrap();
        assert!(
            image
                .windows(b"OSLoadFilename=/unix".len())
                .any(|bytes| bytes == b"OSLoadFilename=/unix")
        );
        image[0x4040] ^= 1;
        let bad = image.clone();
        assert!(o2_environment(&mut image, NETBSD).is_err());
        assert_eq!(image, bad);
    }

    #[test]
    fn octane_preserves_firmware_and_appends_only_changed_variables() {
        let mut image = crate::prombinip30::PROM_IP30_REV4_9.to_vec();
        let original = image.clone();
        octane_environment(&mut image, NETBSD).unwrap();
        assert_eq!(&image[..0xf0018], &original[..0xf0018]);
        let once = image.clone();
        octane_environment(&mut image, NETBSD).unwrap();
        assert_eq!(image, once);
        octane_environment(&mut image, IRIX).unwrap();
        let once = image.clone();
        octane_environment(&mut image, IRIX).unwrap();
        assert_eq!(image, once);
        // Every unrelated record, including firmware logs and stale records,
        // remains byte-for-byte intact at its original address.
        let mut at = 0xf0018;
        while &original[at..at + 2] != [0xff, 0xff] {
            let kind = u16::from_be_bytes(original[at..at + 2].try_into().unwrap());
            let end = at + 4 + (kind & 0xff) as usize * 2;
            if kind & 0xff00 != 0x0100 || variable(&original[at + 4..end]).is_none() {
                assert_eq!(&image[at..end], &original[at..end]);
            }
            at = end;
        }
        image[0xf0000] = 0;
        let bad = image.clone();
        assert!(octane_environment(&mut image, NETBSD).is_err());
        assert_eq!(image, bad);
    }
}
