use crate::platform::timer::TickFrequency;
use std::arch::asm;

/// Reads the cntvct_el0 counter and returns the value.
pub fn ticks() -> u64 {
    let counter: u64;
    unsafe {
        asm!("mrs x0, cntvct_el0", out("x0") counter);
    }
    counter
}

/// We can use the actual ticks value as our start value.
pub fn start() -> u64 {
    ticks()
}

/// We can use the actual ticks value as our stop value.
pub fn stop() -> u64 {
    ticks()
}

/// Our frequency is provided by cntfrq_el0 on the platform.
pub fn frequency() -> TickFrequency {
    let frequency: u64;
    unsafe {
        asm!(
            "mrs x0, cntfrq_el0",
            out("x0") frequency
        );
    }
    TickFrequency::Hardware(frequency)
}
