//! Firmware selection for the O2 and Octane flash devices.

use std::{fs, io::ErrorKind, path::Path};

/// An empty configured path selects embedded bytes without probing the working
/// directory. The shared default probes the supplied paths in order, then uses
/// the embedded image. An explicit override must be readable: never hide a typo
/// or a permission error by silently booting different firmware.
pub(crate) fn load_flash_prom(
    configured: &str,
    defaults: &[&Path],
    embedded: &[u8],
    model: &str,
) -> Result<Vec<u8>, String> {
    if !configured.is_empty() {
        if configured != crate::config::DEFAULT_PROM {
            return fs::read(configured).map_err(|e| format!("{model} PROM {configured}: {e}"));
        }
        for path in defaults {
            match fs::read(path) {
                Ok(bytes) => return Ok(bytes),
                Err(e) if e.kind() == ErrorKind::NotFound => continue,
                Err(e) => return Err(format!("{model} PROM {}: {e}", path.display())),
            }
        }
    }
    crate::diagnostics::stderr(format_args!("{model} PROM: using embedded firmware"));
    Ok(embedded.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check_selection(embedded: &[u8], model: &str) {
        let dir = std::env::temp_dir().join(format!(
            "iris-flash-prom-{model}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
        ));
        fs::create_dir(&dir).unwrap();
        let generic = dir.join("prom.bin");
        let specific = dir.join("model.bin");
        let explicit = dir.join("override.bin");
        let defaults = [generic.as_path(), specific.as_path()];
        let load = |configured| load_flash_prom(configured, &defaults, embedded, model);

        assert_eq!(load(crate::config::DEFAULT_PROM).unwrap(), embedded);
        assert_eq!(load("").unwrap(), embedded);
        assert!(load(explicit.to_str().unwrap())
            .unwrap_err()
            .contains("override.bin"));
        fs::write(&specific, b"model").unwrap();
        assert_eq!(load(crate::config::DEFAULT_PROM).unwrap(), b"model");
        fs::write(&generic, b"generic").unwrap();
        assert_eq!(load(crate::config::DEFAULT_PROM).unwrap(), b"generic");
        fs::write(&explicit, b"override").unwrap();
        assert_eq!(load(explicit.to_str().unwrap()).unwrap(), b"override");
        assert_eq!(load("").unwrap(), embedded);

        // A directory cannot be read as a ROM, even with an embedded fallback.
        assert!(load(dir.to_str().unwrap()).is_err());
        fs::remove_file(&generic).unwrap();
        fs::create_dir(&generic).unwrap();
        assert!(load(crate::config::DEFAULT_PROM).is_err());
        assert_eq!(load("").unwrap(), embedded);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn o2_firmware_selection() {
        check_selection(&crate::prombinip32::PROM_IP32_REV4_18, "O2");
    }

    #[test]
    fn octane_firmware_selection() {
        check_selection(&crate::prombinip30::PROM_IP30_REV4_9, "Octane");
    }
}
