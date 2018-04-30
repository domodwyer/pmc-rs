#[derive(Debug, PartialEq)]
/// `Scope` defines how an event is measured (single process or system-wide).
pub enum Scope {
	/// `System` counters track their event over all processes in the system.
	///
	/// This requires the user to be root.
	System,

	/// `Process` counters track their event for a single process.
	///
	/// A counter must be [attached] to a target when operating in `Process` mode.
	///
	/// [attached]: struct.Counter.html#method.attach
	Process,
}
