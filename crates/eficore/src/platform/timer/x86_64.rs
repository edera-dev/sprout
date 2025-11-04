use crate::platform::timer::TickFrequency;
use core::time::Duration;

/// We will measure the frequency of the timer based on 1000 microseconds.
/// This will result in a call to BS->Stall(1000) in the end.
const MEASURE_FREQUENCY_DURATION: Duration = Duration::from_micros(1000);

/// Read the number of ticks from the platform timer.
pub fn ticks() -> u64 {
    // SAFETY: Reads the platform timer, which is safe in any context.
    unsafe { core::arch::x86_64::_rdtsc() }
}

/// Measure the frequency of the platform timer.
/// NOTE: Intentionally, we do not synchronize rdtsc during measurement to match systemd behavior.
fn measure_frequency() -> u64 {
    let start = ticks();
    uefi::boot::stall(MEASURE_FREQUENCY_DURATION);
    let stop = ticks();
    let elapsed = stop.wrapping_sub(start) as f64;
    (elapsed / MEASURE_FREQUENCY_DURATION.as_secs_f64()) as u64
}

/// Acquire the platform timer frequency.
/// On x86_64, this is slightly expensive, so it should be done once.
pub fn frequency() -> TickFrequency {
    let frequency = measure_frequency();
    TickFrequency::Measured(frequency)
}
