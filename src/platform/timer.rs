// Referenced https://github.com/sheroz/tick_counter (MIT license) as a baseline.
// Architecturally modified to support UEFI and remove x86 (32-bit) support.

use std::time::Duration;

/// Support for aarch64 timers.
#[cfg(target_arch = "aarch64")]
pub mod aarch64;

/// Support for x86_64 timers.
#[cfg(target_arch = "x86_64")]
pub mod x86_64;

/// The tick frequency of the platform.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TickFrequency {
    /// The platform provides the tick frequency.
    Hardware(u64),
    /// The tick frequency is measured internally.
    Measured(u64, Duration),
}

impl TickFrequency {
    /// Acquire the tick frequency reported by the platform.
    fn ticks(&self) -> u64 {
        match self {
            TickFrequency::Hardware(frequency) => *frequency,
            TickFrequency::Measured(frequency, _) => *frequency,
        }
    }

    /// Calculate the nanoseconds represented by a tick.
    fn nanos(&self) -> f64 {
        1.0e9_f64 / (self.ticks() as f64)
    }

    /// Produce a duration from the provided elapsed `ticks` value.
    fn duration(&self, ticks: u64) -> Duration {
        let accuracy = self.nanos();
        let nanos = ticks as f64 * accuracy;
        Duration::from_nanos(nanos as u64)
    }
}

/// Acquire the tick value reported by the platform.
fn arch_ticks() -> u64 {
    #[cfg(target_arch = "aarch64")]
    return aarch64::ticks();
    #[cfg(target_arch = "x86_64")]
    return x86_64::ticks();
}

/// Acquire the tick frequency reported by the platform.
fn arch_frequency() -> TickFrequency {
    #[cfg(target_arch = "aarch64")]
    return aarch64::frequency();
    #[cfg(target_arch = "x86_64")]
    return x86_64::frequency();
}

/// Platform timer that allows measurement of the elapsed time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlatformTimer {
    /// The start tick value.
    start: u64,
    /// The tick frequency of the platform.
    frequency: TickFrequency,
}

impl PlatformTimer {
    /// Start a platform timer at the current instant.
    pub fn start() -> Self {
        Self {
            start: arch_ticks(),
            frequency: arch_frequency(),
        }
    }

    /// Measure the elapsed duration since the hardware started ticking upwards.
    pub fn elapsed_since_lifetime(&self) -> Duration {
        self.frequency.duration(arch_ticks())
    }

    /// Measure the elapsed duration since the timer was started.
    pub fn elapsed_since_start(&self) -> Duration {
        let duration = arch_ticks().wrapping_sub(self.start);
        self.frequency.duration(duration)
    }
}
