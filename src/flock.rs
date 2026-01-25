use super::*;

use std::{
    fs::File,
    sync::{
        Arc, Mutex, MutexGuard,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, SystemTime},
};

static LOCK: Mutex<()> = Mutex::new(());

pub(crate) struct Lock {
    intraprocess_guard: Guard,
    lockfile: FileLock,
}

// High-quality lock to coordinate different #[test] functions within the *same*
// integration test crate.
enum Guard {
    NotLocked,
    Locked(#[allow(dead_code)] MutexGuard<'static, ()>),
}

// Best-effort filesystem lock to coordinate different #[test] functions across
// *different* integration tests.
enum FileLock {
    NotLocked,
    Locked {
        path: PathBuf,
        done: Arc<AtomicBool>,
    },
}

impl Lock {
    pub fn acquire(path: impl AsRef<Path>) -> Result<Self> {
        Ok(Lock {
            intraprocess_guard: Guard::acquire(),
            lockfile: FileLock::acquire(path)?,
        })
    }
}

impl Guard {
    fn acquire() -> Self {
        Guard::Locked(LOCK.lock().unwrap_or_else(|e| e.into_inner()))
    }
}

impl FileLock {
    fn acquire(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_owned();
        let Some(lockfile) = create(&path) else {
            return Ok(FileLock::NotLocked);
        };
        let done = Arc::new(AtomicBool::new(false));
        let thread = std::thread::Builder::new().name("err_span_check-flock".to_owned());
        thread.spawn({
            let done = Arc::clone(&done);
            move || poll(lockfile, done)
        })?;
        Ok(FileLock::Locked { path, done })
    }
}

impl Drop for Lock {
    fn drop(&mut self) {
        let Lock {
            intraprocess_guard,
            lockfile,
        } = self;
        // Unlock file lock first.
        *lockfile = FileLock::NotLocked;
        *intraprocess_guard = Guard::NotLocked;
    }
}

impl Drop for FileLock {
    fn drop(&mut self) {
        match self {
            FileLock::NotLocked => {}
            FileLock::Locked { path, done } => {
                done.store(true, Ordering::Release);
                let _ = std::fs::remove_file(path);
            }
        }
    }
}

fn create(path: &Path) -> Option<File> {
    loop {
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
        {
            // Acquired lock by creating lockfile.
            Ok(lockfile) => return Some(lockfile),
            Err(io_error) => match io_error.kind() {
                // Lock is already held by another test.
                std::io::ErrorKind::AlreadyExists => {}
                // File based locking isn't going to work for some reason.
                _ => return None,
            },
        }

        // Check whether it's okay to bust the lock.
        let metadata = match std::fs::metadata(path) {
            Ok(metadata) => metadata,
            Err(io_error) => match io_error.kind() {
                // Other holder of the lock finished. Retry.
                std::io::ErrorKind::NotFound => continue,
                _ => return None,
            },
        };

        let Ok(modified) = metadata.modified() else {
            return None;
        };

        let now = SystemTime::now();
        let considered_stale = now - Duration::from_millis(1500);
        let considered_future = now + Duration::from_millis(1500);
        if modified < considered_stale || considered_future < modified {
            return File::create(path).ok();
        }

        // Try again shortly.
        std::thread::sleep(Duration::from_millis(500));
    }
}

// Bump mtime periodically while test directory is in use.
fn poll(lockfile: File, done: Arc<AtomicBool>) {
    loop {
        std::thread::sleep(Duration::from_millis(500));
        if done.load(Ordering::Acquire) || lockfile.set_len(0).is_err() {
            return;
        }
    }
}
