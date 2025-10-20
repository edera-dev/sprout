<div align="center">

![Sprout Logo](assets/logo.png)

# Sprout

</div>

Sprout is an **EXPERIMENTAL** programmable UEFI bootloader written in Rust.

Sprout is in use at Edera today in development environments and is intended to ship to production soon.

The name "sprout" is derived from our company name "Edera" which means "ivy."
Given that Sprout is the first thing intended to start on an Edera system, the name was apt.

It supports `x86_64` and `ARM64` EFI-capable systems. It is designed to require UEFI and can be chainloaded from an
existing UEFI bootloader or booted by the hardware directly.

Sprout is licensed under Apache 2.0 and is open to modifications and contributions.

## Documentation

- [Fedora Setup Guide]
- [Development Guide]
- [Contributing Guide]
- [Sprout License]
- [Code of Conduct]
- [Security Policy]

## Features

NOTE: Currently, Sprout is experimental and is not intended for production use. For example, it doesn't currently
have secure boot support. In fact, as of writing, it doesn't even have a boot menu. Instead, it boots the first entry it sees, or fails.

### Current

- [x] Loadable driver support
- [x] [Bootloader specification (BLS)](https://uapi-group.org/specifications/specs/boot_loader_specification/) support
- [x] Chainload support
- [x] Linux boot support via EFI stub
- [x] Load Linux initrd from disk
- [x] Boot first configured entry

### Roadmap

- [ ] Boot menu
- [ ] Secure Boot support: work in progress
- [ ] UKI support: partial
- [ ] Windows boot support (untested via chainload)
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
# that contains \loader\entries as a directory
# into the value called "boot"
[extractors.boot.filesystem-device-match]
has-item = "\\loader\\entries"

# use the sprout bls module to scan a bls
# directory for entries and load them as boot
# entries in sprout, using the entry template
# as specified here. the bls action below will
# be passed the extracted values from bls.
[generators.boot.bls]
path = "$boot\\loader\\entries"
entry.title = "$title"
entry.actions = ["bls"]

# the action that is used for each bls entry above.
[actions.bls]
chainload.path = "$boot\\$chainload"
chainload.options = ["$options"]
chainload.linux-initrd = "$boot\\$initrd"
```

[Fedora Setup Guide]: ./docs/fedora-setup.md
[Development Guide]: ./DEVELOPMENT.md
[Contributing Guide]: ./CONTRIBUTING.md
[Sprout License]: ./LICENSE
[Code of Conduct]: ./CODE_OF_CONDUCT.md
[Security Policy]: ./SECURITY.md
