//! Keep the machine awake while encoding, and clean up orphaned ffmpeg processes.
use std::sync::Mutex;

pub struct SleepGuard {
    #[cfg(target_os = "macos")]
    child: Mutex<Option<std::process::Child>>,
    #[cfg(windows)]
    stop: Mutex<Option<std::sync::mpsc::Sender<()>>>,
    #[cfg(not(any(target_os = "macos", windows)))]
    _p: Mutex<()>,
}

impl Default for SleepGuard {
    fn default() -> Self {
        Self::new()
    }
}

impl SleepGuard {
    pub fn new() -> Self {
        Self {
            #[cfg(target_os = "macos")]
            child: Mutex::new(None),
            #[cfg(windows)]
            stop: Mutex::new(None),
            #[cfg(not(any(target_os = "macos", windows)))]
            _p: Mutex::new(()),
        }
    }

    pub fn acquire(&self) {
        #[cfg(target_os = "macos")]
        {
            let mut g = self.child.lock().unwrap();
            if g.is_none() {
                // -i: prevent idle sleep; -w: exit when our process exits.
                let pid = std::process::id().to_string();
                *g = std::process::Command::new("caffeinate").args(["-i", "-w", &pid]).spawn().ok();
            }
        }
        #[cfg(windows)]
        {
            let mut g = self.stop.lock().unwrap();
            if g.is_none() {
                let (tx, rx) = std::sync::mpsc::channel::<()>();
                std::thread::spawn(move || {
                    use windows_sys::Win32::System::Power::{SetThreadExecutionState, ES_CONTINUOUS, ES_SYSTEM_REQUIRED};
                    unsafe { SetThreadExecutionState(ES_CONTINUOUS | ES_SYSTEM_REQUIRED) };
                    let _ = rx.recv();
                    unsafe { SetThreadExecutionState(ES_CONTINUOUS) };
                });
                *g = Some(tx);
            }
        }
    }

    pub fn release(&self) {
        #[cfg(target_os = "macos")]
        {
            if let Some(mut c) = self.child.lock().unwrap().take() {
                let _ = c.kill();
                let _ = c.wait();
            }
        }
        #[cfg(windows)]
        {
            if let Some(tx) = self.stop.lock().unwrap().take() {
                let _ = tx.send(());
            }
        }
    }
}

/// Kill a process by pid if it is still alive and looks like ffmpeg. Returns true if killed.
pub fn kill_orphan_ffmpeg(pid: u32) -> bool {
    use sysinfo::{Pid, ProcessesToUpdate, System};
    let mut sys = System::new();
    let target = Pid::from_u32(pid);
    sys.refresh_processes(ProcessesToUpdate::Some(&[target]), true);
    if let Some(p) = sys.process(target) {
        let name = p.name().to_string_lossy().to_ascii_lowercase();
        if name.contains("ffmpeg") {
            return p.kill();
        }
    }
    false
}

#[cfg(unix)]
pub fn suspend(pid: u32) {
    unsafe {
        libc::kill(pid as i32, libc::SIGSTOP);
    }
}
#[cfg(unix)]
pub fn resume(pid: u32) {
    unsafe {
        libc::kill(pid as i32, libc::SIGCONT);
    }
}
#[cfg(not(unix))]
pub fn suspend(_pid: u32) {}
#[cfg(not(unix))]
pub fn resume(_pid: u32) {}
