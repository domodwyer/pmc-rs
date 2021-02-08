#![allow(non_camel_case_types)]
#![allow(non_upper_case_globals)]

pub type pmc_id_t = u32;

pub const pmc_mode_PMC_MODE_SC: u32 = 1;
pub const pmc_mode_PMC_MODE_TC: u32 = 2;

pub const EDOOFUS: i32 = 88;

pub unsafe extern "C" fn pmc_allocate(
    _ctrspec: *const i8,
    _mode: u32,
    _flags: u32,
    _cpu: i32,
    _pmcid: *mut u32,
    _count: u64,
) -> i32 {
    unimplemented!("only implemented on FreeBSD")
}

pub unsafe extern "C" fn pmc_attach(_pmcid: u32, _pid: i32) -> i32 {
    unimplemented!("only implemented on FreeBSD")
}

pub unsafe extern "C" fn pmc_detach(_pmcid: u32, _pid: i32) -> i32 {
    unimplemented!("only implemented on FreeBSD")
}

pub unsafe extern "C" fn pmc_read(_pmc: u32, _value: *mut u64) -> i32 {
    unimplemented!("only implemented on FreeBSD")
}

pub unsafe extern "C" fn pmc_release(_pmc: u32) -> i32 {
    unimplemented!("only implemented on FreeBSD")
}

pub unsafe extern "C" fn pmc_rw(_pmc: u32, _newvalue: u64, _oldvalue: *mut u64) -> i32 {
    unimplemented!("only implemented on FreeBSD")
}

pub unsafe extern "C" fn pmc_start(_pmc: u32) -> i32 {
    unimplemented!("only implemented on FreeBSD")
}

pub unsafe extern "C" fn pmc_stop(_pmc: u32) -> i32 {
    unimplemented!("only implemented on FreeBSD")
}

pub unsafe extern "C" fn pmc_init() -> i32 {
    unimplemented!("only implemented on FreeBSD")
}
