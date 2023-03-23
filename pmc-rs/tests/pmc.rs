use pmc::*;

#[test]
fn test_process_counter() {
    let mut counter = CounterBuilder::default()
        .attach_to(vec![0])
        .allocate("ex_ret_instr")
        .expect("failed to allocate PMC");

    read_counter(&mut counter);
}

#[test]
fn test_system_counter() {
    let mut counter = CounterBuilder::default()
        .allocate("ex_ret_instr")
        .expect("failed to allocate PMC");

    read_counter(&mut counter);
}

#[test]
fn test_set_counter() {
    let mut counter = CounterBuilder::default()
        .attach_to(vec![0])
        .allocate("ex_ret_instr")
        .expect("failed to allocate PMC");

    counter.set(42).expect("failed to set counter");
    assert_eq!(counter.read().unwrap(), 42);
    assert_eq!(counter.set(4242).unwrap(), 42);
}

#[test]
fn test_counter_bad_name() {
    let err = CounterBuilder::default()
        .attach_to(vec![0])
        .allocate("inst_retired.any")
        .expect_err("expected to fail allocating PMC");

    assert_eq!(err.kind(), &ErrorKind::AllocInit);
}

#[test]
fn test_null_in_counter_name() {
    let err = CounterBuilder::default()
        .attach_to(vec![0])
        .allocate("instru\0ctions")
        .expect_err("expected to fail allocating PMC");

    assert_eq!(err.kind(), &ErrorKind::InvalidEventSpec);
}

fn read_counter(c: &mut pmc::Counter) {
    let handle = c.start().expect("failed to start counter");

    let mut last: u64 = 0;
    for _ in 1..100 {
        let now = handle.read().expect("unable to read counter");
        if now < last {
            panic!("counter decremented")
        }
        last = now;
    }
}
