# Sprout Development Guide

This guide is a work in progress.

## Development Setup

You can use any Rust development environment to develop Sprout.

Rustup is recommended as the Rust toolchain manager to manage Rust versions and targets.

Sprout currently requires Rust nightly to support uefi_std. See [uefi_std](https://doc.rust-lang.org/beta/rustc/platform-support/unknown-uefi.html) for more details.

We currently only support `x86_64-unknown-uefi` and `aarch64-unknown-uefi` targets.

To test your changes in QEMU, please run `./hack/dev/boot.sh`, you can specify `x86_64` or `aarch64`
as an argument to boot.sh to boot the specified architecture.

## Hack Scripts

You can use the `./hack` scripts to run common development tasks:

### ./hack/build.sh

Builds the Sprout binary for the target that would support your current machine.

### ./hack/assemble.sh

Builds both x86_64 and aarch64 binaries into `target/assemble`.

### ./hack/clean.sh

Cleans the target directory and any docker images that were built.

### ./hack/format.sh

Formats the code using `rustfmt` and shell scripts with `shfmt`.

### ./hack/autofix.sh

Applies Clippy and `rustfmt` fixes to the code, and formats shell scripts with `shfmt`.

## Dev Scripts

### ./hack/dev/build.sh

Build Sprout as OCI images using Docker, including a kernel, initramfs, xen, and other supporting files.

### ./hack/dev/boot.sh

Boot Sprout's dev environment using QEMU for testing. This will let you test your changes in a real environment booting
Alpine Linux with an initramfs.
