use std::ffi::CString;
use std::io;
use std::sync::{Mutex, Once};

#[cfg(target_os = "freebsd")]
use libc::EDOOFUS;
#[cfg(target_os = "freebsd")]
use pmc_sys::{
    pmc_allocate, pmc_attach, pmc_detach, pmc_id_t, pmc_init, pmc_mode_PMC_MODE_SC,
    pmc_mode_PMC_MODE_TC, pmc_read, pmc_release, pmc_rw, pmc_start, pmc_stop,
};

#[cfg(not(target_os = "freebsd"))]
use super::stubs::*;

use crate::CPU_ANY;
use crate::{
    error::{new_error, new_os_error, Error, ErrorKind},
    signal,
};

static PMC_INIT: Once = Once::new();

lazy_static! {
    static ref BIG_FAT_LOCK: Mutex<u32> = Mutex::new(42);
}

/// Configure event counter parameters.
///
/// Unless specified, a counter is allocated in counting mode with a system-wide
/// scope, recording events across all CPUs.
///
/// ```no_run
/// let config = CounterConfig::default().attach_to(vec![0]);
///
/// let instr = config.allocate("inst_retired.any")?;
/// let l1_hits = config.allocate("mem_load_uops_retired.l1_hit")?;
/// #
/// # Ok::<(), Error>(())
/// ```
#[derive(Debug, Default, Clone)]
pub struct CounterConfig {
    cpu: Option<i32>,
    pids: Option<Vec<i32>>,
}

impl CounterConfig {
    /// Specify the CPU number that the PMC is to be allocated on.
    ///
    /// Defaults to all CPUs ([`CPU_ANY`]).
    pub fn set_cpu(self, cpu: i32) -> Self {
        Self {
            cpu: Some(cpu),
            ..self
        }
    }

    /// Attach a counter to the specified PID(s).
    ///
    /// When set, this causes the PMC to be allocated in process-scoped counting
    /// mode ([`pmc_mode_PMC_MODE_TC`] - see `man pmc`).
    ///
    /// # PID 0
    ///
    /// PID 0 is a magic value, attaching to it causes the counter to be
    /// attached to the current (caller's) PID.
    pub fn attach_to(self, pids: impl Into<Vec<i32>>) -> Self {
        Self {
            pids: Some(pids.into()),
            ..self
        }
    }

    /// Allocate a PMC with the specified configuration, and attach to the
    /// target PIDs (if any).
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
        // BUG: do not attempt to detach from pid 0 or risk live-locking the
        // machine.
        //
        //      https://bugs.freebsd.org/bugzilla/show_bug.cgi?id=227041
        //
        if self.pid != 0 {
            unsafe { pmc_detach(self.id, self.pid) };
        }
    }
}

/// A handle to a running PMC counter.
///
/// Dropping this handle causes the counter to stop recording events.
pub struct Running<'a> {
    counter: &'a mut Counter,
}

impl<'a> Running<'a> {
    /// Read the current counter value.
    ///
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

    /// Set the value of the counter.
    pub fn set(&mut self, value: u64) -> Result<u64, Error> {
        self.counter.set(value)
    }

    /// Stop the counter from recording new events.
    pub fn stop(self) {
        drop(self)
    }
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

/// An allocated PMC counter.
///
/// Counters are initialised using the [`CounterBuilder`] type.
///
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
        if unsafe {
            pmc_allocate(
                c_spec.as_ptr(),
                pmc_mode,
                0,
                cpu.unwrap_or(CPU_ANY),
                &mut id,
                0,
            )
        } != 0
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

    /// Start this counter.
    ///
    /// The counter stops when the returned [`Running`] handle is dropped.
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

    /// Read the counter value.
    ///
    /// This call is valid for both running, stopped, and unused counters.
    ///
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

    /// Set an explicit counter value.
    ///
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
            let err = io::Error::last_os_error();
            return match io::Error::raw_os_error(&err) {
                Some(libc::EBUSY) => panic!("{}", err.to_string()),
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
