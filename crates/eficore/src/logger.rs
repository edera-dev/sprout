//! Based on: https://github.com/rust-osdev/uefi-rs/blob/main/uefi/src/helpers/logger.rs

use alloc::format;
use core::fmt::Write;
use core::ptr;
use core::sync::atomic::{AtomicPtr, Ordering};
use log::{Log, Record};
use uefi::proto::console::text::Output;

/// The global logger object.
static LOGGER: Logger = Logger::new();

/// Logging mechanism for Sprout.
/// Must be initialized to be used, as we use atomic pointers to store the output to write to.
pub struct Logger {
    writer: AtomicPtr<Output>,
}

impl Default for Logger {
    /// Creates a default logger, which is uninitialized with an output.
    fn default() -> Self {
        Self::new()
    }
}

impl Logger {
    /// Create a new logger with an output not specified.
    /// This will cause the logger to not print anything until it is configured.
    pub const fn new() -> Self {
        Self {
            writer: AtomicPtr::new(ptr::null_mut()),
        }
    }

    /// Retrieves the pointer to the output.
    /// SAFETY: This pointer might be null, it should be checked before use.
    #[must_use]
    fn output(&self) -> *mut Output {
        self.writer.load(Ordering::Acquire)
    }

    /// Sets the output to write to.
    ///
    /// # Safety
    /// This function is unsafe because the output is technically leaked and unmanaged.
    pub unsafe fn set_output(&self, output: *mut Output) {
        self.writer.store(output, Ordering::Release);
    }
}

impl Log for Logger {
    /// Enable the logger always.
    fn enabled(&self, _metadata: &log::Metadata<'_>) -> bool {
        true
    }

    /// Log the specified `record` to the output if one is set.
    fn log(&self, record: &Record) {
        // Acquire the output. If one is not set, we do nothing.
        let Some(output) = (unsafe { self.output().as_mut() }) else {
            return;
        };

        // Format the log message.
        let message = format!("{}", record.args());

        // Iterate over every line, formatting the message and writing it to the output.
        for line in message.lines() {
            // The format writes the log level in front of every line of text.
            let _ = writeln!(output, "[{:>5}] {}", record.level(), line);
        }
    }

    /// This log is not buffered, so flushing isn't required.
    fn flush(&self) {}
}

/// Initialize the logging environment, calling panic if something goes wrong.
pub fn init() {
    // Retrieve the stdout handle and set it as the output for the global logger.
    uefi::system::with_stdout(|stdout| unsafe {
        // SAFETY: We are using the stdout handle to create a pointer to the output.
        // The handle is global and is guaranteed to be valid for the lifetime of the program.
        LOGGER.set_output(stdout);
    });

    // Set the logger to the global logger.
    if let Err(error) = log::set_logger(&LOGGER) {
        panic!("unable to set logger: {}", error);
    }

    // Set the max level to the level specified by the log features.
    log::set_max_level(log::STATIC_MAX_LEVEL);
}
