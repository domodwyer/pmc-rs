#![allow(missing_docs)]

use std::{error, fmt, io};
use std::borrow::Borrow;

#[derive(Debug)]
pub struct Error {
	kind: ErrorKind,
	cause: Option<Box<dyn error::Error>>,
}

#[derive(Debug, PartialEq)]
pub enum ErrorKind {
	/// An unknown error
	Unknown,

	/// The signal handler received an unrecognised signal.
	UnexpectedSignal,

	/// Failed to initialise [`libpmc`].
	///
	/// [`libpmc`]: https://www.freebsd.org/cgi/man.cgi?query=pmc
	Init,

	/// The system CPU does not support performance monitor counters.
	Unsupported,

	/// The kernel PMC interface differs from what this crate is using.
	///
	/// This usually means FreeBSD/hwpmc has been updated - recompiling the
	/// application might help.
	VersionMismatch,

	/// The provided event specification is not recognised.
	InvalidEventSpec,

	/// `AllocInit` is returned for generic `Counter` initialisation errors, and
	/// unfortunately can be caused by other errors (such as
	/// [`InvalidEventSpec`]) without providing any more information.
	///
	/// [`InvalidEventSpec`]: #variant.InvalidEventSpec
	///
	AllocInit,

	/// The [`hwpmc`] kernel module has been unloaded.
	///
	/// In testing, this signal was not sent from the [`hwpmc`] implementation,
	/// so this error should not be relied upon.
	///
	/// [`hwpmc`]: https://www.freebsd.org/cgi/man.cgi?query=hwpmc
	///
	Unloaded,

	/// A [`Process` scoped] counter is not attached to a target process.
	///
	/// [`Process`]: enum.Scope.html#variant.Process
	///
	NotAttached,

	/// The [`Counter`] is already attached to the requested process.
	///
	/// [`Counter`]: struct.Counter.html
	AlreadyAttached,

	/// The requested [scope] is invalid for the requested event.
	///
	/// [scope]: enum.Scope.html
	///
	BadScope,

	/// The requested event requires a configured log file to write results to.
	LogFileRequired,

	/// The requested operation cannot be performed on a running [`Counter`].
	///
	/// [`Counter`]: struct.Counter.html
	///
	Running,

	/// The requested operation can only be performed on a running [`Counter`].
	///
	/// [`Counter`]: struct.Counter.html
	///
	NotRunning,

	/// The requested target PID is already being monitored by another process.
	BusyTarget,

	/// The requested target PID does not exist.
	BadTarget,

	/// The caller does not have the appropriate permissions.
	Forbidden,
}

impl error::Error for Error {
	fn description(&self) -> &str {
		match self.kind {
			ErrorKind::Init => "missing hwpmc in kernel",
			ErrorKind::Unloaded => "hwpmc unloaded from kernel",
			ErrorKind::Unsupported => "unsupported CPU",
			ErrorKind::VersionMismatch => "unexpected hwpmc version",
			ErrorKind::Running => "cannot set running counter",
			ErrorKind::NotRunning => "counter not running",
			ErrorKind::AllocInit => "failed to allocate counter",
			ErrorKind::BusyTarget => "target is busy",
			ErrorKind::BadTarget => "target PID does not exist",
			ErrorKind::NotAttached => "PMC not attached to target processes",
			ErrorKind::AlreadyAttached => "PMC already attached to target process",
			ErrorKind::Forbidden => "forbidden",
			_ => "unknown error",
		}
	}

	fn cause(&self) -> Option<&dyn error::Error> {
		match self.cause {
			None => None,
			Some(ref b) => Some(b.borrow()),
		}
	}
}

impl fmt::Display for Error {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "{}", self)
	}
}

#[doc(hidden)]
impl PartialEq for Error {
	fn eq(&self, other: &Error) -> bool {
		self.kind == other.kind
	}
}

impl Error {
	pub fn kind(&self) -> &ErrorKind {
		&self.kind
	}
}

pub(crate) fn new_os_error(kind: ErrorKind) -> Error {
	// Get the last OS error to reference as the cause
	Error {
		kind,
		cause: Some(Box::new(io::Error::last_os_error())),
	}
}

pub(crate) fn new_error(kind: ErrorKind) -> Error {
	Error { kind, cause: None }
}
