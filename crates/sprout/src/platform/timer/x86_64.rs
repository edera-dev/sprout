use crate::platform::timer::TickFrequency;
use core::arch::asm;
use core::time::Duration;

/// We will measure the frequency of the timer based on 1000 microseconds.
/// This will result in a call to BS->Stall(1000) in the end.
const MEASURE_FREQUENCY_DURATION: Duration = Duration::from_micros(1000);

/// Read the number of ticks from the platform timer.
pub fn ticks() -> u64 {
    let mut eax: u32;
    let mut edx: u32;

    unsafe {
        asm!("rdtsc", out("eax") eax, out("edx") edx);
    }

    (edx as u64) << 32 | eax as u64
}

/// Read the starting number of ticks from the platform timer.
pub fn start() -> u64 {
    let rax: u64;
    unsafe {
        asm!(
            "mfence",
            "lfence",
            "rdtsc",
            "shl rdx, 32",
            "or rax, rdx",
            out("rax") rax
        );
    }
    rax
}

/// Read the ending number of ticks from the platform timer.
pub fn stop() -> u64 {
    let rax: u64;
    unsafe {
        asm!(
        "rdtsc",
        "lfence",
        "shl rdx, 32",
        "or rax, rdx",
        out("rax") rax
        );
    }
    rax
}

/// Measure the frequency of the platform timer.
fn measure_frequency() -> u64 {
    let start = start();
    uefi::boot::stall(MEASURE_FREQUENCY_DURATION);
    let stop = stop();
    let elapsed = stop.wrapping_sub(start) as f64;
    (elapsed / MEASURE_FREQUENCY_DURATION.as_secs_f64()) as u64
}

/// Acquire the platform timer frequency.
/// On x86_64, this is slightly expensive, so it should be done once.
pub fn frequency() -> TickFrequency {
    let frequency = measure_frequency();
    TickFrequency::Measured(frequency, MEASURE_FREQUENCY_DURATION)
}
