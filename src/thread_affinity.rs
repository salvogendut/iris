//! Optional host thread affinity for emulator worker threads.

use parking_lot::RwLock;

use crate::config::PerfConfig;

static PERF: RwLock<Option<PerfConfig>> = RwLock::new(None);

/// Configure before constructing a workstation, after stopping the previous one.
pub fn init(perf: PerfConfig) {
    *PERF.write() = Some(perf);
}

#[derive(Clone, Copy, Debug)]
pub enum PerfRole {
    MipsCpu,
    Rex3Processor,
    Rex3Refresh,
}

/// Pin the current thread when `[perf] thread_affinity` is enabled.
pub fn pin_current(role: PerfRole) {
    let Some(perf) = PERF.read().clone() else { return; };
    if !perf.thread_affinity {
        return;
    }
    let core = match role {
        PerfRole::MipsCpu => perf.cpu_core,
        PerfRole::Rex3Processor => perf.rex3_core,
        PerfRole::Rex3Refresh => perf.refresh_core,
    };
    let Some(core) = core else { return; };
    if apply_core(core) {
        eprintln!("iris: pinned {:?} thread to core {}", role, core);
    }
}

#[cfg(windows)]
fn apply_core(core: u32) -> bool {
    if core >= usize::BITS { return false; }
    let mask = 1usize << core;
    unsafe {
        windows_sys::Win32::System::Threading::SetThreadAffinityMask(
            windows_sys::Win32::System::Threading::GetCurrentThread(),
            mask,
        ) != 0
    }
}

#[cfg(target_os = "linux")]
fn apply_core(core: u32) -> bool {
    if core as usize >= libc::CPU_SETSIZE as usize { return false; }
    use std::mem::MaybeUninit;
    let mut set: libc::cpu_set_t = unsafe { MaybeUninit::zeroed().assume_init() };
    unsafe {
        libc::CPU_ZERO(&mut set);
        libc::CPU_SET(core as usize, &mut set);
        libc::sched_setaffinity(0, std::mem::size_of_val(&set), &set) == 0
    }
}

#[cfg(all(unix, not(target_os = "linux")))]
fn apply_core(_core: u32) -> bool {
    false
}

#[cfg(not(any(windows, unix)))]
fn apply_core(_core: u32) -> bool {
    false
}

/// One-line summary for `perf snapshot`.
pub fn status_line() -> String {
    let Some(perf) = PERF.read().clone() else {
        return "thread_affinity: (not initialized)".to_string();
    };
    if !perf.thread_affinity {
        return "thread_affinity: off".to_string();
    }
    format!(
        "thread_affinity: on  cpu={:?} rex3={:?} refresh={:?}",
        perf.cpu_core, perf.rex3_core, perf.refresh_core
    )
}

/// Host processor IDs available to the calling thread (not a processor count).
/// Query on an unpinned frontend/worker thread. Unsupported platforms return None.
pub fn available_cores() -> Option<Vec<u32>> {
    #[cfg(target_os = "linux")]
    {
        let mut set: libc::cpu_set_t = unsafe { std::mem::zeroed() };
        if unsafe { libc::sched_getaffinity(0, std::mem::size_of_val(&set), &mut set) } != 0 {
            return None;
        }
        Some((0..libc::CPU_SETSIZE as u32).filter(|&id| unsafe { libc::CPU_ISSET(id as usize, &set) }).collect())
    }
    #[cfg(windows)]
    {
        use windows_sys::Win32::System::Threading::{GetCurrentProcess, GetProcessAffinityMask};
        let (mut process, mut system) = (0usize, 0usize);
        if unsafe { GetProcessAffinityMask(GetCurrentProcess(), &mut process, &mut system) } == 0 {
            return None;
        }
        Some((0..usize::BITS).filter(|id| process & (1usize << id) != 0).collect())
    }
    #[cfg(not(any(target_os = "linux", windows)))]
    { None }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn repeated_configuration_replaces_previous_machine() {
        // Other machine tests initialize this process-wide setting too.
        if std::env::var_os("IRIS_AFFINITY_TEST_CHILD").is_none() {
            assert!(std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "thread_affinity::tests::repeated_configuration_replaces_previous_machine"])
                .env("IRIS_AFFINITY_TEST_CHILD", "1").status().unwrap().success());
            return;
        }
        init(PerfConfig { thread_affinity: true, cpu_core: Some(3), ..Default::default() });
        assert!(status_line().contains("cpu=Some(3)"));
        init(PerfConfig::default());
        assert_eq!(status_line(), "thread_affinity: off");
    }
    #[test]
    fn invalid_core_does_not_reach_host_mask() {
        assert!(!apply_core(u32::MAX));
    }
}
