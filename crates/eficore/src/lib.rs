//! Sprout EFI core.
//! This crate provides core EFI functionality for Sprout.

// For some reason this triggers, and I can't figure out why.
#![allow(rustdoc::bare_urls)]
#![no_std]
extern crate alloc;

/// EFI handle helpers.
pub mod handle;

/// Load and start EFI images.
pub mod loader;

/// Logging support for EFI applications.
pub mod logger;

/// Disk partitioning support infrastructure.
pub mod partition;

/// Path handling for UEFI.
pub mod path;

/// platform: Integration or support code for specific hardware platforms.
pub mod platform;

/// Secure Boot support.
pub mod secure;

/// Support for the shim loader application that enables Secure Boot.
pub mod shim;

/// String utilities.
pub mod strings;

/// Implements support for the bootloader interface specification.
pub mod bootloader_interface;
/// Acquire arguments from UEFI environment.
pub mod env;
/// Support code for the EFI framebuffer.
pub mod framebuffer;
/// Support code for the media loader protocol.
pub mod media_loader;
/// setup: Code that initializes the UEFI environment for Sprout.
pub mod setup;
/// Support code for EFI variables.
pub mod variables;
