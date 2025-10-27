<div align="center">

![Sprout Logo](assets/logo-small.png)

# Sprout

</div>

Sprout is an **EXPERIMENTAL** programmable UEFI bootloader written in Rust.

Sprout is in use at Edera today in development environments and is intended to ship to production soon.

The name "sprout" is derived from our company name "Edera" which means "ivy."
Given that Sprout is the first thing intended to start on an Edera system, the name was apt.

It supports `x86_64` and `ARM64` EFI-capable systems. It is designed to require UEFI and can be chainloaded from an
existing UEFI bootloader or booted by the hardware directly.

Sprout is licensed under Apache 2.0 and is open to modifications and contributions.

## Background

At [Edera] we make compute isolation technology for a wide variety of environments, often ones we do not fully control.
Our technology utilizes a hypervisor to boot the host system to provide a new isolation mechanism that works
with or without hardware virtualization support. To do this we need to inject our hypervisor at boot time.

Unfortunately, GRUB, the most common bootloader on Linux systems today, utilizes a shell-script like
configuration system. Both the code that runs to generate a GRUB config and the GRUB config
itself is fully turing complete. This makes modifying boot configuration difficult and error-prone.

Sprout was designed to take in a machine-readable, writable, and modifiable configuration that treats boot information
like data plus configuration, and can be chained from both UEFI firmware and GRUB alike.

Sprout aims to be flexible, secure, and modern. Written in Rust, it handles data safely and uses unsafe code as little
as possible. It also critically must be easy to install into all common distributions, relying on simple principles to
simplify installation and usage.

## Documentation

- [Fedora Setup Guide]
- [Generic Linux Setup Guide]
- [Alpine Edge Setup Guide]
- [Windows Setup Guide]
- [Development Guide]
- [Contributing Guide]
- [Sprout License]
- [Code of Conduct]
- [Security Policy]

## Features

NOTE: Currently, Sprout is experimental and is not intended for production use.
The boot menu mechanism is very rudimentary.

### Current

- [x] Loadable driver support
- [x] [Bootloader specification (BLS)](https://uapi-group.org/specifications/specs/boot_loader_specification/) support
- [x] Chainload support
- [x] Linux boot support via EFI stub
- [x] Windows boot support via chainload
- [x] Load Linux initrd from disk
- [x] Basic boot menu

### Roadmap

- [ ] Full-featured boot menu
- [ ] Secure Boot support: work in progress
- [ ] UKI support: partial
- [ ] multiboot2 support
- [ ] Linux boot protocol (boot without EFI stub)

## Concepts

- drivers: loadable EFI modules that can add functionality to the EFI system.
- actions: executable code with a configuration that can be run by various other sprout concepts.
- generators: code that can generate boot entries based on inputs or runtime code.
- extractors: code that can extract values from the EFI environment.
- values: key-value pairs that can be specified in the configuration or provided by extractors or generators.
- entries: boot entries that will be displayed to the user.
- phases: stages of the bootloader that can be hooked to run actions at specific points.

## Usage

Sprout is provided as a single EFI binary called `sprout.efi`.
It can be chainloaded from GRUB or other UEFI bootloaders or booted into directly.
Sprout will look for \sprout.toml in the root of the EFI partition it was loaded from.
See [Configuration](#configuration) for how to configure sprout.

## Configuration

Sprout is configured using a TOML file at `\sprout.toml` on the root of the EFI partition sprout was booted from.

### Command Line Options

Sprout supports some command line options that can be combined to modify behavior without the configuration file.

```bash
# Boot Sprout with a specific configuration file.
$ sprout.efi --config=\path\to\config.toml
# Boot a specific entry, bypassing the menu.
$ sprout.efi --boot="Boot Xen"
```

### Boot Linux from ESP

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

### Bootloader Specification (BLS) Support

```toml
# sprout configuration: version 1
version = 1

# load an EFI driver for ext4.
[drivers.ext4]
path = "\\sprout\\drivers\\ext4.efi"

# extract the full path of the first filesystem
# that contains \loader as a directory
# into the value called "boot"
[extractors.boot.filesystem-device-match]
has-item = "\\loader"

# use the sprout bls module to scan a bls
# directory for entries and load them as boot
# entries in sprout, using the entry template
# as specified here. the bls action below will
# be passed the extracted values from bls.
[generators.boot.bls]
path = "$boot\\loader"
entry.title = "$title"
entry.actions = ["bls"]

# the action that is used for each bls entry above.
[actions.bls]
chainload.path = "$boot\\$chainload"
chainload.options = ["$options"]
chainload.linux-initrd = "$boot\\$initrd"
```

[Edera]: https://edera.dev
[Fedora Setup Guide]: ./docs/fedora-setup.md
[Generic Linux Setup Guide]: ./docs/generic-linux-setup.md
[Alpine Edge Setup Guide]: ./docs/alpine-edge-setup.md
[Windows Setup Guide]: ./docs/windows-setup.md
[Development Guide]: ./DEVELOPMENT.md
[Contributing Guide]: ./CONTRIBUTING.md
[Sprout License]: ./LICENSE
[Code of Conduct]: ./CODE_OF_CONDUCT.md
[Security Policy]: ./SECURITY.md
