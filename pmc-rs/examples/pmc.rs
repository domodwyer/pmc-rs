use std::thread;
use std::time::Duration;

use pmc::*;

fn main() {
    let mut counter = CounterBuilder::default()
        // PID 0 is a special argument used to attach to the current process.
        //
        // If you don't specify a PID, a system-wide counter is allocated.
        .attach_to(vec![0])
        .allocate("inst_retired.any")
        .expect("failed to allocate PMC");

    // Start the counter.
    //
    // Dropping the handle (or calling stop()) will pause the counter. It can be
    // resumed by calling start() again.
    let handle = counter.start().expect("failed to start counter");

    for i in 1..10 {
        // Do some stuff...
        //
        // The handle implements Display, printing the current counter value.
        println!("iteration {}: {}", i, handle);
        thread::sleep(Duration::from_millis(100));
    }

    // Stop the counter by dropping the handle or calling stop:
    handle.stop();

    println!("retired instructions: {}", counter.read().unwrap());
}
