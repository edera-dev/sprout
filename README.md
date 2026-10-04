<div align="center">

![Sprout Logo](assets/logo-small.png)

# Sprout

</div>

Sprout is a programmable UEFI bootloader written in Rust.

It is in use at Edera today in development environments and is intended to ship to production soon.

The name "Sprout" is derived from our company name "Edera" which means "ivy."
Given that Sprout is the first thing intended to start on an Edera system, the name was apt.

It supports `x86_64` and `ARM64` EFI-capable systems. It is designed to require UEFI and can be chainloaded from an
existing UEFI bootloader or booted by the hardware directly.

Sprout is licensed under Apache 2.0 and is open to modifications and contributions.

**NOTE**: Sprout is still in beta. Some features may not work as expected.
Please [report any bugs you find](https://github.com/edera-dev/sprout/issues/new/choose).

## Background

At [Edera] we make compute isolation technology for a wide variety of environments, often ones we do not fully control.
Our technology uses a hypervisor to boot the host system to provide a new isolation mechanism that works
with or without hardware virtualization support. To do this, we need to inject our hypervisor at boot time.

Unfortunately, GRUB, the most common bootloader on Linux systems today, uses a shell-script like
configuration system. Both the code that runs to generate a GRUB config and the GRUB config
itself is fully turing complete. This makes modifying boot configuration difficult and error-prone.

Sprout was designed to take in a machine-readable, writable, and modifiable configuration that treats boot information
like data plus configuration, and can be chained from both UEFI firmware and GRUB alike.

Sprout aims to be flexible, secure, and modern. Written in Rust, it handles data safely and uses unsafe code as little
as possible. It also critically must be easy to install into all common distributions, relying on simple principles to
simplify installation and usage.

## Documentation

### Setup Guides

Some guides support Secure Boot and some do not.
We recommend running Sprout without Secure Boot for development, and with Secure Boot for production.

| Operating System | Secure Boot Enabled | Link                                                  |
|------------------|---------------------|-------------------------------------------------------|
| Fedora           | ✅                   | [Setup Guide](./docs/setup/signed/fedora.md)          |
| Debian           | ✅                   | [Setup Guide](./docs/setup/signed/debian.md)          |
| Ubuntu           | ✅                   | [Setup Guide](./docs/setup/signed/ubuntu.md)          |
| openSUSE         | ✅                   | [Setup Guide](./docs/setup/signed/opensuse.md)        |
| Fedora           | ❌                   | [Setup Guide](./docs/setup/unsigned/fedora.md)        |
| Alpine Edge      | ❌                   | [Setup Guide](./docs/setup/unsigned/alpine-edge.md)   |
| Generic Linux    | ❌                   | [Setup Guide](./docs/setup/unsigned/generic-linux.md) |
| Windows          | ❌                   | [Setup Guide](./docs/setup/unsigned/windows.md)       |

### Project Documentation

- [Configuration Reference]
- [Development Guide]
- [Contributing Guide]
- [Sprout License]
- [Code of Conduct]
- [Security Policy]

## Features

### Current

- [x] Loadable driver support
- [x] [Bootloader specification (BLS)](https://uapi-group.org/specifications/specs/boot_loader_specification/) support:
  Type #1 entries, Type #2 unified kernel images, and the extended boot loader partition
- [x] [UKI support](https://github.com/edera-dev/sprout/issues/6): beta, including images with multiple profiles
- [x] Boot counting, so `systemd-bless-boot` can assess a boot
- [x] `loader.conf` support
- [x] Chainload support
- [x] Linux boot support via EFI stub
- [x] Windows boot support via chainload
- [x] Load Linux initrd from disk
- [x] Devicetree support
- [x] Basic, simple, and graphical boot menus
- [x] A strict mode that follows the BLS specification and systemd-boot exactly
- [x] Generators for BLS entries, lists, and matrices, with variants
- [x] BLS autoconfiguration support
- [x] [Secure Boot support](https://github.com/edera-dev/sprout/issues/20): beta
- [x] [Bootloader interface support](https://github.com/edera-dev/sprout/issues/21): beta
- [x] [BLS specification conformance](https://github.com/edera-dev/sprout/issues/2): beta

### Roadmap

- [ ] [Full-featured boot menu](https://github.com/edera-dev/sprout/issues/1)
- [ ] Network boot of unified kernel images (`uki-url`) and devicetree overlays
- [ ] A random seed for the Linux kernel
- [ ] [multiboot2 support](https://github.com/edera-dev/sprout/issues/7)
- [ ] [Linux boot protocol (boot without EFI stub)](https://github.com/edera-dev/sprout/issues/8)

## Concepts

- drivers: loadable EFI modules that can add functionality to the EFI system.
- autoconfiguration: code that can automatically generate sprout.toml based on the EFI environment.
- actions: executable code with a configuration that can be run by various other sprout concepts.
- generators: code that can generate boot entries based on inputs or runtime code.
- extractors: code that can extract values from the EFI environment.
- values: key-value pairs that can be specified in the configuration or provided by extractors or generators.
- entries: boot entries that will be displayed to the user.
- phases: stages of the bootloader that can be hooked to run actions at specific points.

## Usage

Sprout is provided as a single EFI binary called `sprout.efi`.
It can be chainloaded from GRUB or other UEFI bootloaders or booted into directly.
Sprout reads its configuration from `\sprout.toml` in the root of the EFI partition it was loaded from.

This is a configuration that boots a Linux kernel from the EFI partition:

```toml
# sprout configuration: version 1
version = 1

# add a boot entry for booting linux
# which will run the boot-linux action.
[entries.boot-linux]
title = "Boot Linux"
actions = ["boot-linux"]

# use the chainload action to boot linux via the efi stub.
# the options below are passed to the efi stub as the
# kernel command line. the initrd is loaded using the efi stub
# initrd loader mechanism.
[actions.boot-linux]
chainload.path = "\\vmlinuz"
chainload.options = ["root=/dev/sda1"]
chainload.linux-initrd = "\\initrd"
```

On a system that follows the Boot Loader Specification, you may not need to write one. With an ext4 driver
loaded, autoconfiguration finds the entries on its own:

```toml
version = 1

[drivers.ext4]
path = "\\sprout\\drivers\\ext4.efi"

[options]
autoconfigure = true
```

The full list of settings, the command line options, generators, and how Sprout treats BLS entries,
`loader.conf` and strict mode are in [the configuration reference](./docs/config.md).

[Edera]: https://edera.dev
[Configuration Reference]: ./docs/config.md
[Development Guide]: ./DEVELOPMENT.md
[Contributing Guide]: ./CONTRIBUTING.md
[Sprout License]: ./LICENSE
[Code of Conduct]: ./CODE_OF_CONDUCT.md
[Security Policy]: ./SECURITY.md
