#![warn(missing_docs)]

//! `pmc-rs` provides a safe abstraction for interacting with Performance
//! Monitor Counters on [`FreeBSD`].
//!
//! PMCs are part of the CPU hardware and are typically used to profile CPU
//! micro-architecture events such as L1/L2/etc cache hit ratio, instructions
//! processed per CPU tick, TLB lookups, branch mispredictions, etc for a
//! particular application or algorithm. Using PMCs an algorithm can be tuned
//! for performance by minimising CPU stalls, optimising CPU cache usage, etc.
//!
//! The events are defined by the CPU manufacturer (here is the [Intel 64 and
//! IA-32 Architectures Developer's Manual: vol.
//! 3B](https://www.intel.com/content/www/us/en/architecture-and-technology/64-ia-32-architectures-software-developer-vol-3b-part-2-manual.html)
//! where the events can be found in section `18.2.1.2 "Pre-defined
//! Architectural Performance Events"`, `Table 18-1 "UMask and Event Select
//! Encodings for Pre-Defined Architectural Performance Events"`).
//!
//! `pmc-rs` makes use of [`libpmc`] and the [`hwpmc`] kernel module on
//! [`FreeBSD`].
//!
//! [`FreeBSD`]: https://www.freebsd.org/
//! [`hwpmc`]: https://www.freebsd.org/cgi/man.cgi?query=hwpmc
//! [`libpmc`]: https://www.freebsd.org/cgi/man.cgi?query=pmc
//!

#[macro_use]
extern crate lazy_static;
extern crate libc;
extern crate pmc_sys;

mod signal;
pub mod error;

mod scope;
pub use self::scope::Scope;

mod counter;
pub use self::counter::Counter;

use pmc_sys::PMC_CPU_ANY;

/// `Counter` instances allocated with `CPU_ANY` will measure events across all
/// CPUs.
///
/// `CPU_ANY` is a convenience value for readability and should be preferred
/// over using `0` directly.
pub const CPU_ANY: i32 = PMC_CPU_ANY;

// TODO: add sampler type that records to a log file
