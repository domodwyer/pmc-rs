[![crates.io](https://img.shields.io/crates/v/pmc-rs.svg)](https://crates.io/crates/pmc-rs)
[![docs.rs](https://docs.rs/pmc-rs/badge.svg)](https://docs.rs/pmc-rs)

# pmc-rs

`pmc-rs` provides a safe abstraction for interacting with libpmc/hwpmc's
Performance Monitor Counters on [FreeBSD].

PMCs are part of the CPU hardware and are typically used to profile CPU
micro-architecture events such as L1/L2/L3 cache hits & misses, instructions
processed per CPU tick, TLB lookups, branch mispredictions, etc for a particular
application or algorithm. Using PMCs an algorithm can be tuned for performance
by minimising CPU stalls, optimising CPU cache usage, and ensuring the CPU is
always doing useful work.

The events are defined by the CPU manufacturer (here is the [Intel 64 and IA-32
Architectures Developer's Manual: vol. 3B][arch-manual] where the events can be
found in section `18.2.1.2 "Pre-defined Architectural Performance Events"`,
`Table 18-1 "UMask and Event Select Encodings for Pre-Defined Architectural
Performance Events"`).

`pmc-rs` makes use of the [`libpmc`] userland interace to the [`hwpmc`] kernel
module on [FreeBSD].

## Version Compatibility

The latest release of `pmc-rs` generally targets the latest FreeBSD release. Due
to changes in `libpmc` between FreeBSD versions, compatibility with older
FreeBSD versions may require pinning `pmc-rs` to an older release.

| FreeBSD Version | Latest `pmc-rs` Release |
| :-------------- | ----------------------- |
| FreeBSD 11      | v0.1.1                  |
| FreeBSD 12.3+   | v0.2.0                  |
| FreeBSD 13      | v0.2.0                  |

Versions prior to FreeBSD 11 are untested, but may work.

FreeBSD 12.0 linked C++ into the libpmc stack causing compatibility issues
with Rust (see [this issue][freebsd-12-support]). Fortunately it was fixed in
FreeBSD 12.3 and 13.0.

## Future improvements

* Support sampling PMCs.
* Read counters with the `RDPMC` instruction to avoid context switching.

[FreeBSD]: https://www.freebsd.org/
[`hwpmc`]: https://www.freebsd.org/cgi/man.cgi?query=hwpmc
[`libpmc`]: https://www.freebsd.org/cgi/man.cgi?query=pmc
[freebsd-12-support]: https://github.com/domodwyer/pmc-rs/issues/7
[docs]: https://itsallbroken.com/code/docs/pmc-rs/pmc/index.html
[arch-manual]: https://www.intel.com/content/www/us/en/architecture-and-technology/64-ia-32-architectures-software-developer-vol-3b-part-2-manual.html
