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
- [x] Boot counting and automatic boot assessment
- [x] `loader.conf` support
- [x] Chainload support
- [x] Linux boot support via EFI stub
- [x] Windows boot support via chainload
- [x] Load Linux initrd from disk
- [x] Devicetree support
- [x] Basic, simple, and graphical boot menus
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
# Autoconfigure Sprout, without loading a configuration file.
$ sprout.efi --autoconfigure
# Use the basic boot menu instead of the simple one.
$ sprout.efi --menu-style=basic
# Use the graphical boot menu, which can be used with the mouse.
$ sprout.efi --menu-style=graphical
# Show the boot menu for 10 seconds before booting the default entry.
$ sprout.efi --menu-timeout=10
# Show the boot menu even if an entry was chosen with --boot.
$ sprout.efi --force-menu
# Keep the boot console as it is when an entry is booted.
$ sprout.efi --retain-boot-console
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

# global options.
[options]
# enable autoconfiguration by detecting bls enabled
# filesystems and generating boot entries for them.
autoconfigure = true
```

Sprout reads Type #1 entries from `\loader\entries` and Type #2 unified kernel images (UKIs) from
`\EFI\Linux`, and sorts them as the specification says. A unified kernel image with several profiles
is one entry for each profile. Entries that are not for the architecture of the machine are hidden.
Type #1 entries can use `linux`, `efi`, `uki`, `initrd`, `options`, `devicetree`, `architecture`,
`profile`, `sort-key`, `version`, `machine-id`, and `title`.

To set up the generator by hand instead of using autoconfiguration, add a generator and an action
that boots the entries. The entry values `$chainload`, `$options`, `$initrd-0` to `$initrd-7`,
`$devicetree`, `$cmdline`, `$title`, `$version`, and `$entry-root` are available.

```toml
[generators.bls]
# the directory that has the entries directory. this is the default.
bls.path = "\\loader"
# the directory of unified kernel images. by default, this is \EFI\Linux on
# the device of the path. an empty path turns unified kernel images off.
bls.uki-path = "\\EFI\\Linux"
# also read the extended boot loader partition (XBOOTLDR) of the same disk,
# which is sorted with the other entries. its files are on that partition, so
# the action has to use $entry-root, which is empty for Sprout's own partition.
bls.xbootldr = false
# keep the name of the entry file as the name of the entry, so it matches
# the ids that bootctl uses.
bls.pin-names = true
bls.entry.title = "$title"
bls.entry.actions = ["boot-bls"]

[actions.boot-bls]
chainload.path = "$entry-root\\$chainload"
chainload.options = ["$options"]
chainload.devicetree = "$entry-root\\$devicetree"
chainload.linux-initrd-chain = [
  "$entry-root\\$initrd-0",
  "$entry-root\\$initrd-1",
]
```

An entry that names a `devicetree` boots with that devicetree installed for the image, which is put
back when the image returns. It is not used when Secure Boot is enabled, as it can't be verified.

#### Boot counting

An entry file named like `fedora+3.conf` or `fedora+3.efi` has three tries. Each time Sprout boots it, the
file is renamed, such as to `fedora+2-1.conf`, and the new path is given to the system in
`LoaderBootCountPath` so `systemd-bless-boot` can remove the counter once the boot works.
Entries that have no tries left are sorted last and are not picked as the default entry, but they can still be
booted by hand. If an entry with tries left fails to start, the machine resets, so the next boot can use
the next try or another entry. This is set by `reboot-on-error` in `loader.conf`.

#### loader.conf

Sprout reads `\loader\loader.conf` from the partition it was loaded from, as systemd-boot does.

| Key               | Value                                                              |
|-------------------|--------------------------------------------------------------------|
| `default`         | A pattern for the id of the default entry, or `@saved`.            |
| `preferred`       | Like `default`, but entries with no boot counter tries are skipped. |
| `timeout`         | Seconds, `menu-hidden`, `menu-disabled`, or `menu-force`.           |
| `reboot-on-error` | `auto` (the default), `yes`, or `no`.                               |

The id of an entry is the name of its file without the boot counter, such as `fedora.conf` or `fedora.efi`.
Patterns ignore case and can use `*`, `?`, and `[a-z]`. With `@saved`, the entry that was booted last is the
default. With `reboot-on-error`, `yes` always resets after an entry fails to start, which can loop forever,
and `auto` only does when a boot counter try was used up and there were tries left.
Other keys are ignored with a warning.

A hidden menu, from a timeout of zero or `menu-hidden`, still opens when a key is pressed. `menu-disabled` does not.

#### Bootloader interface

Sprout uses the same variables as systemd-boot, so `bootctl`, `systemctl reboot --boot-loader-entry`, and
`systemd-bless-boot` can work with it. It publishes `LoaderEntries`, `LoaderEntrySelected`,
`LoaderBootCountPath`, `LoaderFeatures`, and `LoaderInfo`, and reads `LoaderEntryDefault`,
`LoaderEntryPreferred`, `LoaderEntryOneShot`, `LoaderEntryLastBooted`, `LoaderConfigTimeout`,
and `LoaderConfigTimeoutOneShot`.

The default entry comes from the first of these that matches an entry:

1. `default-entry` in `sprout.toml`
2. `LoaderEntryPreferred`, then `preferred` in `loader.conf`
3. `LoaderEntryDefault`, then `default` in `loader.conf`

The menu timeout comes from the first of the one-shot timeout, `--menu-timeout`, `menu-timeout` in
`sprout.toml`, `LoaderConfigTimeout`, and `loader.conf`.

### Generators

Generators make entries when Sprout starts. The `matrix` generator makes an entry for every combination of
its values, the `list` generator makes an entry for each item of a list, and the `bls` generator makes
entries from BLS files. Variants multiply the entries of any generator, and `exclude` removes some of them.

```toml
# makes an entry for each kernel and each console, such as "Boot \vmlinuz (serial)".
[generators.kernels]
matrix.entry.title = "Boot $kernel ($console)"
matrix.entry.actions = ["boot-kernel"]
matrix.values.kernel = ["\\vmlinuz", "\\vmlinuz-lts"]
variants.console = [
  { name = "serial", values.console-options = "console=ttyS0" },
  { name = "graphics", values.console-options = "console=tty0" },
]

[actions.boot-kernel]
chainload.path = "$kernel"
chainload.options = ["$console-options"]
```

[Edera]: https://edera.dev
[Development Guide]: ./DEVELOPMENT.md
[Contributing Guide]: ./CONTRIBUTING.md
[Sprout License]: ./LICENSE
[Code of Conduct]: ./CODE_OF_CONDUCT.md
[Security Policy]: ./SECURITY.md
