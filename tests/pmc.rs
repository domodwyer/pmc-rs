extern crate pmc;

use pmc::error::*;

#[test]
fn test_process_counter() {
    let mut counter = pmc::Counter::new("PAGE_FAULT.ALL", &pmc::Scope::Process, pmc::CPU_ANY)
        .expect("failed to create counter");

    read_counter(&mut counter);
}

#[test]
#[ignore]
fn test_system_counter() {
    let mut counter =
        pmc::Counter::new("cycles", &pmc::Scope::System, 0).expect("failed to create counter");

    read_counter(&mut counter);
}

#[test]
fn test_set_counter() {
    let mut counter = pmc::Counter::new("LOCK.FAILED", &pmc::Scope::Process, pmc::CPU_ANY)
        .expect("failed to create counter");

    counter.set(42).expect("failed to set counter");
    assert_eq!(counter.read().unwrap(), 42);
    assert_eq!(counter.set(4242).unwrap(), 42);
}

#[test]
fn test_counter_bad_name() {
    assert_eq!(
        pmc::Counter::new("wat", &pmc::Scope::Process, pmc::CPU_ANY)
            .unwrap_err()
            .kind(),
        &ErrorKind::AllocInit
    );
}

#[test]
fn test_null_in_counter_name() {
    assert_eq!(
        pmc::Counter::new("instru\0ctions", &pmc::Scope::Process, pmc::CPU_ANY)
            .unwrap_err()
            .kind(),
        &ErrorKind::InvalidEventSpec
    );
}

#[test]
fn test_attach_to_pid() {
    let mut counter = pmc::Counter::new("instructions", &pmc::Scope::Process, pmc::CPU_ANY)
        .expect("failed to create counter");

    // pmc_attach treats 0 as "attach to self"
    counter.attach(0).expect("failed to attach to self");

    read_counter(&mut counter);
}

fn read_counter(c: &mut pmc::Counter) {
    c.start().expect("failed to start counter");

    let mut last: u64 = 0;
    for _ in 1..100 {
        let now = c.read().expect("unable to read counter");
        if now < last {
            panic!("counter decremented")
        }
        last = now;
    }
}
