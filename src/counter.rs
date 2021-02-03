extern crate libc;
extern crate pmc_sys;

use error::{new_error, new_os_error, Error, ErrorKind};
use scope::Scope;
use signal;

use std::io;
use std::ffi::CString;
use std::sync::{Mutex, Once};
use pmc_sys::{pmc_mode_PMC_MODE_SC, pmc_mode_PMC_MODE_TC};
use pmc_sys::{pmc_allocate, pmc_attach, pmc_id_t, pmc_mode, pmc_read, pmc_release, pmc_rw,
              pmc_start, pmc_stop};

static SIGNAL_WATCH: Once = Once::new();

lazy_static! {
	static ref ALLOCATE_LOCK: Mutex<i32> = Mutex::new(42);
}

#[derive(Debug)]
/// A `Counter` represents a virtualised, hardware-backed performance monitor
/// counter (PMC).
///
/// # Safety
///
/// `Counter` uses unsafe code and all [`pmc-rs`] code has been thoroughly
/// tested. However the [`hwpmc(4)`] kernel module itself has questionable safety
/// and known bugs - [`pmc-rs`] attempts to work around all known issues but
/// care should be taken.
///
/// # Examples
///
/// ```
/// # extern crate pmc;
/// #
/// # use std::time::Duration;
/// # use std::thread;
/// #
/// # fn test_main() -> Result<(), pmc::error::Error> {
/// let mut counter =
///     pmc::Counter::new("instructions", &pmc::Scope::Process, pmc::CPU_ANY)?;
///
/// // Attach and start the counter.
/// //
/// // PID 0 is a special argument used to attach to the current process
/// counter.attach(0)?;
/// counter.start()?;
///
/// for i in 1..10 {
///     // do some stuff...
///     thread::sleep(Duration::from_millis(100));
/// }
///
/// // Optionally stop the counter - it can be restarted any time
/// counter.stop()?;
///
/// assert!(counter.read().is_ok());
/// assert!(counter.read().unwrap() > 0);
/// #
/// #   Ok(())
/// # }
/// #
/// # fn main() {
/// #   test_main().unwrap();
/// # }
/// ```
///
/// [`hwpmc(4)`]: https://www.freebsd.org/cgi/man.cgi?query=hwpmc
/// [`pmc-rs`]: index.html
///
pub struct Counter<'a> {
	pmc_id: Option<pmc_id_t>,
	event_spec: &'a str,
	pmc_mode: pmc_mode,
	running: bool,
	attached: Vec<u32>,
}

impl<'a> Counter<'a> {
	fn init(&self) -> Result<(), Error> {
		if unsafe { pmc_sys::pmc_init() } == 0 {
			return Ok(());
		}

		// Register the signal handler
		SIGNAL_WATCH.call_once(|| {
			signal::watch_for(&[libc::SIGBUS, libc::SIGIO]);
		});

		match io::Error::raw_os_error(&io::Error::last_os_error()) {
			Some(libc::ENOENT) => Err(new_os_error(ErrorKind::Init)),
			Some(libc::ENXIO) => Err(new_os_error(ErrorKind::Unsupported)),
			Some(libc::EPROGMISMATCH) => Err(new_os_error(ErrorKind::VersionMismatch)),
			_ => Err(new_os_error(ErrorKind::Unknown)),
		}
	}

	/// Allocates a new Counter.
	///
	/// `event_spec` should be an event specifier recognised by the FreeBSD PMC
	/// subsystem, and must be valid for `scope`.
	///
	/// Run `apropos pmc.` on the target system for a full list of supported
	/// CPUs and their event specifications.
	///
	/// `cpu` is either `CPU_ANY` or represents the CPU to allocate the counter
	/// on. Only events occurring on the specified CPU will increment the
	/// counter.
	///
	/// # Examples
	///
	/// Allocate a process-scoped instructions counter, measuring events across
	/// all CPUs in the system:
	/// ```
	/// # extern crate pmc;
	/// #
	/// # fn test_main() -> Result<(), pmc::error::Error> {
	/// let mut counter =
	///     pmc::Counter::new("instructions", &pmc::Scope::Process, pmc::CPU_ANY)?;
	/// #   Ok(())
	/// # }
	/// #
	/// # fn main() {
	/// #   test_main().unwrap();
	/// # }
	/// ```
	///
	///
	/// Allocate a system-scoped cycles counter, measuring events on the first
	/// CPU only (1-based indexing):
	/// ```
	/// # extern crate pmc;
	/// #
	/// # fn test_main() -> Result<(), pmc::error::Error> {
	/// let mut counter =
	///     pmc::Counter::new("cycles", &pmc::Scope::System, 1)?;
	///
	/// #   Ok(())
	/// # }
	/// #
	/// # fn main() {
	/// #   test_main().unwrap();
	/// # }
	/// ```
	///
	pub fn new(event_spec: &'a str, scope: &Scope, cpu: i32) -> Result<Self, Error> {
		let pmc_mode = match *scope {
			Scope::System => pmc_mode_PMC_MODE_SC,
			Scope::Process => pmc_mode_PMC_MODE_TC,
		};

		let mut c = Counter {
			event_spec,
			pmc_mode,
			pmc_id: None,
			running: false,
			attached: Vec::new(),
		};

		// It appears pmc_allocate isn't thread safe, so take a lock while
		// calling it
		let _lock = ALLOCATE_LOCK.lock().unwrap();

		// Initialise libpmc and check for any signals from hwpmc
		c.init()?;
		signal::check()?;

		let spec = CString::new(c.event_spec).map_err(|_| new_error(ErrorKind::InvalidEventSpec))?;

		// Allocate the PMC
		let mut id = 0;
		if unsafe { pmc_allocate(spec.as_ptr(), c.pmc_mode, 0 as u32, cpu, &mut id) } != 0 {
			return match io::Error::raw_os_error(&io::Error::last_os_error()) {
				Some(libc::EINVAL) => Err(new_os_error(ErrorKind::AllocInit)),
				_ => Err(new_os_error(ErrorKind::Unknown)),
			};
		}

		c.pmc_id = Some(id);
		Ok(c)
	}

	/// Attach a [`Process`-scoped] counter to a running process. It is an error
	/// to attach a system-scoped counter to a process.
	///
	/// Attaching to PID `0` is treated as a special request to attach to the
	/// currently running process.
	///
	/// # Permissions
	///
	/// Internally [`p_candebug(9)`] is called to determine if the user is
	/// allowed to attach to the requested process - this basically involves
	/// checking if the sysctl `security.bsd.unprivileged_proc_debug` is
	/// non-zero to allow users to attach, otherwise restricting the call to
	/// root.
	///
	/// # Examples
	///
	/// ```
	/// # extern crate pmc;
	/// #
	/// # fn test_main() -> Result<(), pmc::error::Error> {
	/// # let mut counter =
	/// #    pmc::Counter::new("cycles", &pmc::Scope::System, 0)?;
	/// if let Some(e) = counter.attach(0).err() {
	///     println!("failed to attach to self: {}", e);
	/// }
	/// #   Ok(())
	/// # }
	/// #
	/// # fn main() {
	/// #   test_main().unwrap();
	/// # }
	/// ```
	///
	/// [`p_candebug(9)`]: https://www.freebsd.org/cgi/man.cgi?query=p_candebug&sektion=9
	/// [`Process`-scoped]: enum.Scope.html#variant.Process
	///
	pub fn attach(&mut self, target: u32) -> Result<(), Error> {
		signal::check()?;

		if self.attached.contains(&target) {
			return Err(new_error(ErrorKind::AlreadyAttached));
		}

		if self.running {
			return Err(new_error(ErrorKind::Running));
		}

		if unsafe { pmc_attach(self.pmc_id.unwrap(), target as pmc_sys::pid_t) } != 0 {
			return match io::Error::raw_os_error(&io::Error::last_os_error()) {
				Some(libc::EBUSY) => Err(new_os_error(ErrorKind::BusyTarget)),
				Some(libc::EEXIST) => Err(new_os_error(ErrorKind::AlreadyAttached)),
				Some(libc::EPERM) => Err(new_os_error(ErrorKind::Forbidden)),
				Some(libc::EINVAL) | Some(libc::ESRCH) => Err(new_os_error(ErrorKind::BadTarget)),
				_ => Err(new_os_error(ErrorKind::Unknown)),
			};
		}

		// TODO: remove attached

		self.attached.push(target);
		Ok(())
	}

	#[doc(hidden)]
	#[allow(unreachable_code)]
	#[allow(unused_variables)]
	/// Detach this counter from a process.
	///
	/// It is an error to try and detach this counter from a process it was
	/// never never attached to.
	///
	/// Don't use detach due to a hwpmc bug.
	///
	pub fn detach(&mut self, target: u32) -> Result<(), Error> {
		unimplemented!("https://bugs.freebsd.org/bugzilla/show_bug.cgi?id=227041");

		if !self.attached.contains(&target) {
			return Err(new_error(ErrorKind::BadTarget));
		}

		// if unsafe { pmc_detach(self.pmc_id, target as pmc_sys::pid_t) } != 0 {
		// 	return match io::Error::raw_os_error(&io::Error::last_os_error()) {
		// 		Some(libc::EINVAL) => Err(new_error(ErrorKind::BadTarget)),
		// 		Some(libc::ESRCH) => Err(new_error(ErrorKind::BadTarget)),
		// 		_ => Err(new_os_error(ErrorKind::Unknown)),
		// 	};
		// }

		// Remove any references to target in the attached PID list
		self.attached.retain(|&pid| pid != target);
		Ok(())
	}

	/// Start measuring the `Counter` event.
	///
	/// # Support
	///
	/// Some sampling events require a log file to be configured which is
	/// currently unsupported.
	///
	/// # Examples
	///
	/// ```
	/// # extern crate pmc;
	/// #
	/// # fn test_main() -> Result<(), pmc::error::Error> {
	/// # let mut counter =
	/// #    pmc::Counter::new("cycles", &pmc::Scope::System, 0)?;
	///	if let Some(e) = counter.start().err() {
	/// 	println!("failed to start counter: {}", e);
	/// }
	/// #   Ok(())
	/// # }
	/// #
	/// # fn main() {
	/// #   test_main().unwrap();
	/// # }
	/// ```
	///
	pub fn start(&mut self) -> Result<(), Error> {
		signal::check()?;

		if self.running {
			return Err(new_error(ErrorKind::Running));
		}

		if unsafe { pmc_start(self.pmc_id.unwrap()) } != 0 {
			return match io::Error::raw_os_error(&io::Error::last_os_error()) {
				Some(libc::EDOOFUS) => Err(new_os_error(ErrorKind::LogFileRequired)),
				Some(libc::ENXIO) => Err(new_os_error(ErrorKind::BadScope)),
				_ => Err(new_os_error(ErrorKind::Unknown)),
			};
		}

		self.running = true;
		Ok(())
	}

	/// Stop measuring the `Counter` event.
	pub fn stop(&mut self) -> Result<(), Error> {
		signal::check()?;

		if !self.running {
			return Err(new_error(ErrorKind::NotRunning));
		}

		if unsafe { pmc_stop(self.pmc_id.unwrap()) } != 0 {
			return Err(new_os_error(ErrorKind::Unknown));
		}

		self.running = false;
		Ok(())
	}

	/// Read a `Counter` value.
	///
	/// A `Counter` can be read at any time (not yet started, running, stopped,
	/// etc), however the value only increments while a `Counter` is [running].
	///
	/// # Examples
	///
	/// ```
	/// # extern crate pmc;
	/// #
	/// #
	/// # fn test_main() -> Result<(), pmc::error::Error> {
	/// # let mut counter =
	/// #    pmc::Counter::new("instructions", &pmc::Scope::Process, pmc::CPU_ANY)?;
	///	# counter.attach(0).unwrap();
	/// #
	/// let r1 = counter.read()?;
	/// # counter.set(42).unwrap();
	/// let r2 = counter.read()?;
	///
	/// assert!(r2 > r1);
	/// #
	/// #   Ok(())
	/// # }
	/// #
	/// # fn main() {
	/// #   test_main().unwrap();
	/// # }
	/// ```
	///
	/// [running]: #method.start
	///
	pub fn read(&self) -> Result<u64, Error> {
		signal::check()?;

		let mut value: u64 = 0;
		if unsafe { pmc_read(self.pmc_id.unwrap(), &mut value) } != 0 {
			return Err(new_os_error(ErrorKind::Unknown));
		}

		Ok(value)
	}

	/// Set performs an atomic read-and-set, returning the current counter value
	/// before setting it to value.
	///
	/// A counter that is running cannot be set.
	///
	/// # Examples
	///
	/// ```
	/// # extern crate pmc;
	/// #
	/// # fn test_main() -> Result<(), pmc::error::Error> {
	/// # let mut counter =
	/// #    pmc::Counter::new("instructions", &pmc::Scope::Process, pmc::CPU_ANY)?;
	///	# counter.attach(0).unwrap();
	/// #
	/// # counter.set(42)?;
	/// # counter.start()?;
	/// let r1 = counter.read()?;
	///
	/// // Do some stuff...
	///
	/// counter.stop()?;
	/// # counter.set(4242)?;
	/// let r2 = counter.set(0)?;
	/// let r3 = counter.read()?;
	///
	/// assert!(r2 != 0);
	/// assert!(r2 > r1);
	/// assert!(r3 == 0);
	/// #
	/// #   Ok(())
	/// # }
	/// #
	/// # fn main() {
	/// #   test_main().unwrap();
	/// # }
	/// ```
	///
	pub fn set(&mut self, value: u64) -> Result<u64, Error> {
		signal::check()?;

		let mut old: u64 = 0;
		if unsafe { pmc_rw(self.pmc_id.unwrap(), value, &mut old) } != 0 {
			return match io::Error::raw_os_error(&io::Error::last_os_error()) {
				Some(libc::EBUSY) => Err(new_os_error(ErrorKind::Running)),
				_ => Err(new_os_error(ErrorKind::Unknown)),
			};
		}

		Ok(old)
	}
}

impl<'a> Drop for Counter<'a> {
	fn drop(&mut self) {
		if self.pmc_id.is_none() {
			return;
		}

		// Stop an active PMC counter before releasing
		if self.running {
			let _ = self.stop();
		}

		unsafe {
			pmc_release(self.pmc_id.unwrap());
		}
	}
}
