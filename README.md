[![crates.io](https://img.shields.io/crates/v/pmc-rs.svg)](https://crates.io/crates/pmc-rs)

`pmc-rs` provides a safe abstraction for interacting with Performance
Monitor Counters on [FreeBSD].

PMCs are part of the CPU hardware and are typically used to profile CPU
micro-architecture events such as L1/L2/etc cache hit ratio, instructions
processed per CPU tick, TLB lookups, branch mispredictions, etc for a particular
application or algorithm. Using PMCs an algorithm can be turned for performance
by minimising CPU stalls, optimising CPU cache usage, etc.

The events are defined by the CPU manufacturer (here is the [Intel 64 and
IA-32 Architectures Developer's Manual: vol.
3B](https://www.intel.com/content/www/us/en/architecture-and-technology/64-ia-32-architectures-software-developer-vol-3b-part-2-manual.html)
where the events can be found in section `18.2.1.2 "Pre-defined
Architectural Performance Events"`, `Table 18-1 "UMask and Event Select
Encodings for Pre-Defined Architectural Performance Events"`).

`pmc-rs` makes use of the [`libpmc`] userland interace to the [`hwpmc`] kernel
module on [FreeBSD].

The documentation can be found
[here](https://itsallbroken.com/code/docs/pmc-rs/pmc/index.html) as `docs.rs`
doesn't build FreeBSD only packages.

[FreeBSD]: https://www.freebsd.org/
[`hwpmc`]: https://www.freebsd.org/cgi/man.cgi?query=hwpmc
[`libpmc`]: https://www.freebsd.org/cgi/man.cgi?query=pmc

Future improvements
----

* Support sampling PMCs.
* Read counters with the `RDPMC` instruction to avoid context switching.
* Split `Counter` into `ProcessCounter` and `SystemCounter` to enforce invariants specific to each scope using the type system.