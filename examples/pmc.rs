extern crate pmc;

use std::time::Duration;
use std::thread;

fn main() {
	let mut counter =
		pmc::Counter::new("instructions", &pmc::Scope::Process, pmc::CPU_ANY).unwrap();

	// PID 0 is a special argument used to attach to the calling process
	counter.attach(0).unwrap();

	// Start the counter
	counter.start().unwrap();

	for i in 1..10 {
		// do some stuff...
		println!("{}", i);
		thread::sleep(Duration::from_millis(100));
	}

	// Stop the counter - it can be restarted any time
	counter.stop().unwrap();

	println!("retired instructions: {}", counter.read().unwrap());
}
