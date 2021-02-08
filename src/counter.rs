use std::ffi::CString;
use std::io;
use std::sync::{Mutex, Once};

#[cfg(target_os = "freebsd")]
use libc::EDOOFUS;
#[cfg(target_os = "freebsd")]
use pmc_sys::{
    pid_t, pmc_allocate, pmc_attach, pmc_detach, pmc_id_t, pmc_init, pmc_mode_PMC_MODE_SC,
    pmc_mode_PMC_MODE_TC, pmc_read, pmc_release, pmc_rw, pmc_start, pmc_stop,
};

#[cfg(not(target_os = "freebsd"))]
use super::stubs::*;

use crate::{
    error::{new_error, new_os_error, Error, ErrorKind},
    signal,
};

static PMC_INIT: Once = Once::new();

lazy_static! {
    static ref BIG_FAT_LOCK: Mutex<u32> = Mutex::new(42);
}

// TODO:  docs
/// ```no_run
/// let config = CounterConfig::default().attach_to(vec![0]);
///
/// let instr = config.allocate("inst_retired.any")?;
/// let l1_hits = config.allocate("mem_load_uops_retired.l1_hit")?;
/// #
/// # Ok::<(), Error>(())
/// ```
// TODO: doc default
#[derive(Debug, Default, Clone)]
pub struct CounterConfig {
    cpu: Option<i32>,
    pids: Option<Vec<i32>>,
}

impl CounterConfig {
    pub fn set_cpu(self, cpu: i32) -> Self {
        Self {
            cpu: Some(cpu),
            ..self
        }
    }

    // TODO: 0 pid
    pub fn attach_to(self, pids: impl Into<Vec<i32>>) -> Self {
        Self {
            pids: Some(pids.into()),
            ..self
        }
    }

    pub fn allocate(&self, event_spec: impl Into<String>) -> Result<Counter, Error> {
        Counter::new(event_spec, self.cpu, self.pids.clone())
    }
}

#[derive(Debug)]
struct AttachHandle {
    id: pmc_id_t,
    pid: i32,
}

impl Drop for AttachHandle {
    fn drop(&mut self) {
        unsafe { pmc_detach(self.id, self.pid) };
    }
}

pub struct Running<'a> {
    counter: &'a mut Counter,
}

impl<'a> Running<'a> {
    // TODO: docs
    /// ```no_run
    /// let mut counter = CounterConfig::default()
    ///     .attach_to(vec![0])
    ///     .allocate("inst_retired.any")?;
    ///
    /// let handle = counter.start()?;
    ///
    /// println!("instructions: {}", handle.read()?);
    /// #
    /// # Ok::<(), Error>(())
    /// ```
    pub fn read(&self) -> Result<u64, Error> {
        self.counter.read()
    }

    pub fn set(&mut self, value: u64) -> Result<u64, Error> {
        self.counter.set(value)
    }

    pub fn stop(self) {}
}

impl<'a> std::fmt::Display for Running<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.counter.fmt(f)
    }
}

impl<'a> Drop for Running<'a> {
    fn drop(&mut self) {
        unsafe { pmc_stop(self.counter.id) };
    }
}

// TODO:  docs
/// ```no_run
/// use std::{thread, time::Duration};
///
/// let instr = CounterConfig::default()
///     .attach_to(vec![0])
///     .allocate("inst_retired.any")?;
///
/// let handle = instr.start()?;
///
/// // Stop the counter after 5 seconds
/// thread::sleep(Duration::from_secs(5));
/// handle.stop();
///
/// println!("instructions: {}", instr.read()?);
/// #
/// # Ok::<(), Error>(())
/// ```
#[derive(Debug)]
pub struct Counter {
    id: pmc_id_t,
    attached: Option<Vec<AttachHandle>>,
}

impl Counter {
    fn new(
        event_spec: impl Into<String>,
        cpu: Option<i32>,
        pids: Option<Vec<i32>>,
    ) -> Result<Self, Error> {
        // If there's any pids, request a process counter, otherwise a
        // system-wide counter.
        let pmc_mode = if pids.is_none() {
            pmc_mode_PMC_MODE_SC
        } else {
            pmc_mode_PMC_MODE_TC
        };

        // It appears pmc_allocate isn't thread safe, so take a lock while
        // calling it.
        let _guard = BIG_FAT_LOCK.lock().unwrap();

        init_pmc_once()?;
        signal::check()?;

        let c_spec =
            CString::new(event_spec.into()).map_err(|_| new_error(ErrorKind::InvalidEventSpec))?;

        // Allocate the PMC
        let mut id = 0;
        if unsafe { pmc_allocate(c_spec.as_ptr(), pmc_mode, 0, cpu.unwrap_or(0), &mut id, 0) } != 0
        {
            return match io::Error::raw_os_error(&io::Error::last_os_error()) {
                Some(libc::EINVAL) => Err(new_os_error(ErrorKind::AllocInit)),
                _ => Err(new_os_error(ErrorKind::Unknown)),
            };
        }

        // Initialise the counter so dropping it releases the PMC
        let mut c = Counter { id, attached: None };

        // Attach to pids, if any, and collect handles so dropping them later
        // causes them to detach.
        //
        // The handles MUST be dropped before the Counter instance.
        if let Some(pids) = pids {
            let mut handles = vec![];

            for pid in pids {
                if unsafe { pmc_attach(id, pid) } != 0 {
                    return match io::Error::raw_os_error(&io::Error::last_os_error()) {
                        Some(libc::EBUSY) => unreachable!(),
                        Some(libc::EEXIST) => Err(new_os_error(ErrorKind::AlreadyAttached)),
                        Some(libc::EPERM) => Err(new_os_error(ErrorKind::Forbidden)),
                        Some(libc::EINVAL) | Some(libc::ESRCH) => {
                            Err(new_os_error(ErrorKind::BadTarget))
                        }
                        _ => Err(new_os_error(ErrorKind::Unknown)),
                    };
                }

                handles.push(AttachHandle { id, pid })
            }

            c.attached = Some(handles)
        }

        Ok(c)
    }

    #[must_use = "counter only runs until handle is dropped"]
    pub fn start(&mut self) -> Result<Running<'_>, Error> {
        signal::check()?;

        if unsafe { pmc_start(self.id) } != 0 {
            return match io::Error::raw_os_error(&io::Error::last_os_error()) {
                Some(EDOOFUS) => Err(new_os_error(ErrorKind::LogFileRequired)),
                Some(libc::ENXIO) => Err(new_os_error(ErrorKind::BadScope)),
                _ => Err(new_os_error(ErrorKind::Unknown)),
            };
        }

        Ok(Running { counter: self })
    }

    // TODO: docs
    /// ```no_run
    /// let mut counter = CounterConfig::default()
    ///     .attach_to(vec![0])
    ///     .allocate("inst_retired.any")?;
    ///
    /// let r1 = counter.read()?;
    /// let r2 = counter.read()?;
    ///
    /// // A counter that is not running does not advance
    /// assert!(r2 == r1);
    /// #
    /// # Ok::<(), Error>(())
    /// ```
    pub fn read(&self) -> Result<u64, Error> {
        signal::check()?;

        let mut value: u64 = 0;
        if unsafe { pmc_read(self.id, &mut value) } != 0 {
            return Err(new_os_error(ErrorKind::Unknown));
        }

        Ok(value)
    }

    // TODO: docs
    /// ```no_run
    /// let mut counter = CounterConfig::default()
    ///     .attach_to(vec![0])
    ///     .allocate("inst_retired.any")?;
    ///
    /// let r1 = counter.set(42)?;
    /// // The previous value is returned when setting a new value
    /// assert_eq!(r1, 0);
    ///
    /// // Reading the counter returns the value set
    /// let r2 = counter.read()?;
    /// assert_eq!(r2, 42);
    /// #
    /// # Ok::<(), Error>(())
    /// ```
    pub fn set(&mut self, value: u64) -> Result<u64, Error> {
        signal::check()?;

        let mut old: u64 = 0;
        if unsafe { pmc_rw(self.id, value, &mut old) } != 0 {
            return match io::Error::raw_os_error(&io::Error::last_os_error()) {
                e @ Some(libc::EBUSY) => panic!(e),
                _ => Err(new_os_error(ErrorKind::Unknown)),
            };
        }

        Ok(old)
    }
}

impl std::fmt::Display for Counter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.read() {
            Ok(v) => write!(f, "{}", v),
            Err(e) => write!(f, "error: {}", e),
        }
    }
}

impl Drop for Counter {
    fn drop(&mut self) {
        let _guard = BIG_FAT_LOCK.lock().unwrap();

        // The handles MUST be dropped before the Counter instance
        self.attached = None;

        unsafe {
            pmc_release(self.id);
        }
    }
}

fn init_pmc_once() -> Result<(), Error> {
    let mut maybe_err = Ok(());
    PMC_INIT.call_once(|| {
        if unsafe { pmc_init() } != 0 {
            maybe_err = match io::Error::raw_os_error(&io::Error::last_os_error()) {
                Some(libc::ENOENT) => Err(new_os_error(ErrorKind::Init)),
                Some(libc::ENXIO) => Err(new_os_error(ErrorKind::Unsupported)),
                Some(libc::EPROGMISMATCH) => Err(new_os_error(ErrorKind::VersionMismatch)),
                _ => Err(new_os_error(ErrorKind::Unknown)),
            };
            return;
        }

        // Register the signal handler
        signal::watch_for(&[libc::SIGBUS, libc::SIGIO]);
    });
    maybe_err
}
