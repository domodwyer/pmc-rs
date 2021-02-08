#![allow(dead_code)]

extern crate libc;

use crate::error::{new_error, Error, ErrorKind};
use std::sync::atomic::{AtomicI32, Ordering};

static LAST_SIG: AtomicI32 = AtomicI32::new(0);

extern "C" {
    fn signal(sig: libc::c_int, cb: extern "C" fn(libc::c_int));
}

extern "C" fn interrupt(sig: libc::c_int) {
    LAST_SIG.store(sig, Ordering::SeqCst);
}

pub fn check() -> Result<(), Error> {
    let sig = LAST_SIG.load(Ordering::SeqCst);
    match sig {
        0 => Ok(()),
        e @ libc::SIGIO => unreachable!(!e),
        libc::SIGBUS => Err(new_error(ErrorKind::Unloaded)),
        _ => Err(new_error(ErrorKind::UnexpectedSignal)),
    }
}

pub fn watch_for(sigs: &[libc::c_int]) {
    for sig in sigs {
        unsafe {
            signal(*sig, interrupt);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Used to raise a signal to ourselves during tests
    extern "C" {
        fn raise(sig: libc::c_int) -> libc::c_int;
    }

    #[test]
    fn test_watch_for() {
        LAST_SIG.store(0, Ordering::SeqCst);

        for _ in 0..5 {
            watch_for(&[libc::SIGIO]);
            assert!(check().is_ok(), "got unexpected signal");
        }
    }

    #[test]
    #[ignore]
    fn test_recognised_signal() {
        watch_for(&[libc::SIGBUS]);
        LAST_SIG.store(0, Ordering::SeqCst);

        unsafe {
            raise(libc::SIGBUS);
        }

        assert_eq!(check(), Err(new_error(ErrorKind::Unloaded)));
    }

    #[test]
    #[ignore]
    fn test_unrecognised_signal() {
        watch_for(&[libc::SIGBUS]);
        LAST_SIG.store(0, Ordering::SeqCst);

        unsafe {
            raise(libc::SIGINFO);
        }
        assert!(check().is_ok(), "got unexpected signal");
    }
}
