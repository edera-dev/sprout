# Configuring Sprout

Sprout reads a TOML file, `\sprout.toml`, from the root of the EFI partition it was loaded from. This page
covers everything that file can hold, the command line options that override it, and the BLS behavior that
sits next to it. For installing Sprout, see the [setup guides](./setup).

- [Command line options](#command-line-options)
- [The file at a glance](#the-file-at-a-glance)
- [Values](#values)
- [`options`](#options)
- [`menu-styles`](#menu-styles)
- [`drivers`](#drivers)
- [`extractors`](#extractors)
- [`actions`](#actions)
- [`entries`](#entries)
- [`generators`](#generators)
- [`phases`](#phases)
- [Autoconfiguration](#autoconfiguration)
- [Bootloader Specification support](#bootloader-specification-support)
- [Strict mode](#strict-mode)

Sprout reads the file once at startup. A key it doesn't know is ignored with a warning that names the key, so
a typo shows up in the boot log instead of silently doing nothing. A `version` other than `1` stops Sprout.

## Command line options

These are passed to `sprout.efi` as load options, which is how GRUB or `efibootmgr` hand them over. They
override the matching setting in the file.

| Option                    | What it does                                                                |
|---------------------------|-----------------------------------------------------------------------------|
| `--config=PATH`           | Load a configuration file other than `\sprout.toml`.                        |
| `--autoconfigure`         | Skip the configuration file and build one from what Sprout finds.           |
| `--boot=ENTRY`            | Boot an entry without showing the menu. See [entries](#entries) for matching. |
| `--force-menu`            | Show the menu even when `--boot` picked an entry.                           |
| `--menu-style=STYLE`      | `basic`, `simple` or `graphical`.                                           |
| `--menu-timeout=SECONDS`  | How long the menu waits before booting the default entry.                   |
| `--retain-boot-console`   | Don't clear the screen before handing off to the entry.                     |
| `--bls-strict-mode`       | Turn on [strict mode](#strict-mode).                                        |
| `--help`                  | Print the options.                                                          |

```bash
# Boot a specific configuration file.
$ sprout.efi --config=\path\to\config.toml
# Boot "Boot Xen" at once, but only show the menu if it can't be found.
$ sprout.efi --boot="Boot Xen"
# Show the graphical menu for 10 seconds.
$ sprout.efi --menu-style=graphical --menu-timeout=10
```

An argument Sprout can't parse is dropped with a warning and the rest are used, since a typo in a boot
entry shouldn't leave a machine unbootable.

## The file at a glance

Every section is optional. An empty file is a valid configuration that has no entries.

```toml
version = 1

[options]       # settings for Sprout itself
[menu-styles.x] # settings for a style of menu
[values]        # named strings for use anywhere with $name
[drivers.x]     # EFI drivers to load
[extractors.x]  # values worked out at boot
[actions.x]     # things Sprout can do
[entries.x]     # items in the boot menu
[generators.x]  # makes entries from data
[[phases.early]]    # actions to run at set points
```

The pieces fit together like this: an **entry** is a menu item that runs **actions**; an action is configured
with strings that can contain **values**; values come from the `[values]` table, from **extractors**, and
from **generators**. **Drivers** run first so the filesystems the rest of the file points at exist.

Order at boot:

1. `phases.early`
2. drivers load
3. autoconfiguration, if enabled
4. extractors run
5. `phases.startup`
6. entries and generators produce the menu
7. an entry is chosen
8. `phases.late`, then the entry's actions

## Values

A value is a named string. Anywhere a string is accepted in an entry, action, driver path or phase, `$name`
is replaced with the value called `name`.

```toml
[values]
root-options = "root=/dev/sda2 ro"
kernel = "\\vmlinuz"

[actions.boot]
chainload.path = "$kernel"
chainload.options = ["$root-options", "quiet"]
```

Some details that matter in practice:

- Values can refer to other values. Sprout keeps substituting until nothing changes, so
  `a = "$b"`, `b = "x"` gives `x` for both. A value that never settles, such as one that contains itself,
  is an error once it hits an iteration or length limit.
- When two keys share a prefix, the longer one wins, so `$kernel-options` is never read as `$kernel` plus
  `-options`.
- A `$name` that has no value is left in the string as written.
- Values are layered. A key in a generator or entry overrides the same key set globally, and only for that
  entry. From weakest to strongest: `[values]`, extractor results, generator values (and `variants`), then the
  `values` of the entry itself.
- Windows-style paths need doubled backslashes in TOML strings: `"\\EFI\\Linux"`. Literal strings
  (`'\EFI\Linux'`) work too.

## `options`

```toml
[options]
default-entry = "boot-linux"
menu-timeout = 5
menu-style = "graphical"
autoconfigure = false
bls-strict-mode = false
```

| Key               | Type    | Default  | Meaning                                                                 |
|-------------------|---------|----------|-------------------------------------------------------------------------|
| `default-entry`   | string  | none     | The entry that is selected first, and booted when the timeout ends. Without it, the first entry in the menu is. |
| `menu-timeout`    | integer | see below | Seconds to show the menu before booting the default entry.             |
| `menu-style`      | string  | `simple` | `basic` prints entries and takes a number. `simple` is a full-screen list moved with the arrow keys. `graphical` is a menu drawn on the screen (the mouse is off unless you enable it, see [`menu-styles`](#menu-styles)). |
| `autoconfigure`   | boolean | `false`  | Add entries for what Sprout finds. See [autoconfiguration](#autoconfiguration). |
| `bls-strict-mode` | boolean | `false`  | See [strict mode](#strict-mode).                                        |

When `menu-timeout` is not set anywhere, the menu shows for 10 seconds, or is hidden in strict mode. The
sources are ranked in [The bootloader interface](#the-bootloader-interface).

## `menu-styles`

Settings for a particular menu style. Only the graphical menu has any.

```toml
[menu-styles.graphical]
enable-mouse = true
enable-logo = false
enable-animation = false
```

| Key                | Type    | Default | Meaning                                                          |
|--------------------|---------|---------|------------------------------------------------------------------|
| `enable-mouse`     | boolean | `false` | Select entries with the mouse and show the cursor. Without it, or when the firmware has no pointing device, the menu takes the keyboard only. |
| `enable-logo`      | boolean | `true`  | Show the Sprout logo next to the name. Without it, only the name is shown. |
| `enable-animation` | boolean | `true`  | Bounce the logo. Without it, the logo stays still.               |

## `drivers`

Drivers are EFI executables that add features, most often a filesystem driver for something the firmware
can't read, such as ext4. Each is started once and expected to return. Afterwards Sprout reconnects every
controller so the new filesystems appear. Drivers have no defined load order.

```toml
[drivers.ext4]
path = "\\sprout\\drivers\\ext4.efi"
```

| Key    | Meaning                                                |
|--------|--------------------------------------------------------|
| `path` | Path to the driver image. Required. Can use values.    |

The name (`ext4` above) is only for error messages and doesn't have to match the file.

## `extractors`

An extractor works out a value when Sprout boots, so a configuration doesn't need to hardcode where things
are. The name of the extractor is the name of the value it sets.

There is one kind, `filesystem-device-match`, which finds a filesystem and returns its device root. The
result has a trailing slash, so you append a path to it directly: `$boot\\vmlinuz`.

```toml
[extractors.boot]
filesystem-device-match.has-label = "BOOT"
filesystem-device-match.fallback = "PciRoot(0x0)/Pci(0x1,0x1)/Sata(0,0,0)/HD(1,GPT,c2a4f1b6-0000-0000-0000-000000000000)/"
```

| Key                      | Meaning                                                              |
|--------------------------|----------------------------------------------------------------------|
| `has-label`              | The filesystem volume label equals this.                             |
| `has-item`               | This file or directory exists on the filesystem.                     |
| `has-partition-uuid`     | The unique GUID of the partition.                                    |
| `has-partition-type-uuid`| The partition type GUID, such as the one for XBOOTLDR.               |
| `fallback`               | What to use when no filesystem matches. Without it, no match is an error. |

Every criterion you give must match, and you must give at least one. The first matching filesystem wins.
`has-label` and `has-item` can use values.

## `actions`

An action is a named, configured piece of work. Entries and phases refer to actions by name, and an action
can be used by as many of them as you like.

Each action sets exactly one of `chainload`, `print` or `edera`. If you set more than one Sprout warns and
uses the first of them in that order.

### `chainload`

Loads and starts another EFI image. That covers Linux through its EFI stub, Windows, and any other EFI
application. If the image returns, Sprout returns to its own flow.

```toml
[actions.boot-linux]
chainload.path = "\\vmlinuz"
chainload.options = ["root=/dev/sda1", "quiet"]
chainload.linux-initrd = "\\initrd"
```

| Key                  | Type            | Meaning                                                              |
|----------------------|-----------------|----------------------------------------------------------------------|
| `path`               | string          | The image to start. Required.                                        |
| `options`            | list of strings | Joined with spaces and given to the image as its command line.       |
| `linux-initrd`       | string          | One initrd, handed to the Linux EFI stub through the initrd media loader. |
| `linux-initrd-chain` | list of strings | Several initrds, concatenated in order into one. Can't be used together with `linux-initrd`. |
| `devicetree`         | string          | A flattened devicetree to install until the image returns.           |

Notes:

- `linux-initrd` is better than an `initrd=` option, because it doesn't depend on the stub resolving a path.
- Entries in `linux-initrd-chain` that are empty after substitution are skipped, which is what makes a
  chain with spare slots safe.
- The devicetree is ignored with a warning when Secure Boot is on, as Sprout can't verify it. If it can't be
  installed, Sprout warns and boots without it, unless [strict mode](#strict-mode) is on.
- When the image is a Windows boot manager or another non-Linux image, leave the initrd keys out.

### `print`

Writes a line to the console. It's mostly for phases, such as a message while drivers load.

```toml
[actions.hello]
print.text = "Booting $version"
```

### `edera`

Boots the Edera hypervisor and its root operating system, as an extension of the Xen EFI stub. Sprout hands
the Xen config, dom0 kernel and optional initrd to Xen as media loaders, then starts Xen.

```toml
[actions.edera]
edera.xen = "\\EFI\\edera\\xen.efi"
edera.kernel = "\\EFI\\edera\\kernel"
edera.initrd = "\\EFI\\edera\\initrd"
edera.xen-options = ["dom0_mem=512M"]
edera.kernel-options = ["console=hvc0"]
```

| Key              | Type            | Meaning                                                     |
|------------------|-----------------|-------------------------------------------------------------|
| `xen`            | string          | The Xen EFI image. Required.                                |
| `kernel`         | string          | The dom0 kernel. Required.                                  |
| `initrd`         | string          | The dom0 initrd.                                            |
| `initrd-chain`   | list of strings | Several initrds, concatenated in order. Can't be used with `initrd`. |
| `kernel-options` | list of strings | Command line of the dom0 kernel.                            |
| `xen-options`    | list of strings | Command line of Xen.                                        |

When a TPM is present, the Xen options and dom0 command line are measured into PCR 12, the kernel into
PCR 11, and the initrd into PCR 9.

## `entries`

An entry is something the user can pick from the menu. The table name identifies it.

```toml
[entries.boot-linux]
title = "Boot Linux"
actions = ["boot-linux"]
values.kernel = "\\vmlinuz"
sort-key = "10"
```

| Key        | Type            | Meaning                                                                |
|------------|-----------------|------------------------------------------------------------------------|
| `title`    | string          | The text in the menu. Required, and can use values.                    |
| `actions`  | list of strings | Actions to run, in order, when the entry is chosen.                    |
| `values`   | table           | Values that exist only for this entry.                                 |
| `sort-key` | string          | Orders the entry in the menu. Compared as versions, not as text, so `10` sorts after `9`. Defaults to the entry's name. |

The menu lists entries in reverse sort order, newest first. That is what you want for kernel versions and
is why `sort-key = "$version"` gives a sensible menu.

`--boot` and the bootloader interface find an entry by its name, by its title, or by its position in the
menu counted from 0. A `*` in the pattern matches anything.

If an entry fails Sprout reports the error and waits ten seconds. Then it exits to the firmware, or resets the
machine when `reboot-on-error` applies (see [boot counting](#boot-counting)).

## `generators`

A generator makes entries from data when Sprout starts. Each has a kind (`matrix`, `list` or `bls`), and
optionally `variants` and `exclude`, which work on the result of any kind. Set only one kind per generator.

The entries a generator makes are named `<generator>-<n>`, with `n` counting from 0, and a BLS generator
uses the name of the entry file instead. Every one of them is built from the `entry` template you give,
which takes the same keys as an [entry](#entries).

### `matrix`

Makes an entry for every combination of the lists you give.

```toml
[generators.kernels]
matrix.entry.title = "Boot $kernel with $console"
matrix.entry.actions = ["boot-kernel"]
matrix.values.kernel = ["\\vmlinuz", "\\vmlinuz-lts"]
matrix.values.console = ["ttyS0", "tty0"]
```

That is four entries. A list of two and a list of three give six.

### `list`

Makes one entry per item. Each item is a table of values. Use it when the combinations aren't a clean grid.

```toml
[generators.systems]
list.entry.title = "Boot $name"
list.entry.actions = ["boot-system"]
list.values = [
  { name = "Alpine", kernel = "\\alpine\\vmlinuz" },
  { name = "Debian", kernel = "\\debian\\vmlinuz" },
]
```

### `bls`

Makes entries from the files of the [Bootloader Specification](#bootloader-specification-support).

```toml
[generators.bls]
bls.path = "\\loader"
bls.entry.title = "$title"
bls.entry.actions = ["boot-bls"]
```

| Key          | Type    | Default           | Meaning                                                           |
|--------------|---------|-------------------|-------------------------------------------------------------------|
| `entry`      | table   |                   | The template entry. Required.                                     |
| `path`       | string  | `\loader`         | The directory that holds the `entries` directory. Can include a device, like `$boot\\loader`. |
| `uki-path`   | string  | `\EFI\Linux` on the device of `path` | Where to look for unified kernel images. An empty string turns them off. |
| `xbootldr`   | boolean | `false`           | Also read the extended boot loader partition of the same disk.    |
| `pin-names`  | boolean | `true`            | Keep the entry file's name as the entry name, instead of prefixing the generator name. |

Two generators reading the same directory produce entries with the same name. Sprout warns about it, since
picking by name becomes ambiguous. `pin-names` is what makes names match the ids that `bootctl` uses.

The values each BLS entry provides are listed under [BLS values](#bls-values).

### `variants`

Variants multiply the entries of a generator. Each key is an axis, and each axis is a list of choices.
Every entry is made once for each combination of choices, taking one from each axis.

```toml
[generators.kernels]
matrix.entry.title = "Boot $kernel ($console)"
matrix.entry.actions = ["boot-kernel"]
matrix.values.kernel = ["\\vmlinuz", "\\vmlinuz-lts"]
variants.console = [
  { name = "serial", values.console-options = "console=ttyS0" },
  { name = "graphics", values.console-options = "console=tty0" },
]
```

This makes "Boot \vmlinuz (serial)", "Boot \vmlinuz (graphics)" and the same two for `\vmlinuz-lts`. Four
entries from a matrix of two kernels and two choices.

A choice has a `name` and optional `values`. The axis name becomes a value holding the choice's name
(`$console` is `serial` here), and the choice's own values are added next to it. Names are appended to the
entry name, so the entries above are called `kernels-0-serial`, `kernels-0-graphics` and so on. Axes apply in
alphabetical order, choices in the order written. An axis with no choices, or two choices that share a name,
is an error.

### `exclude`

Drops generated entries. It's a list of rules, and a rule is a table of value names to patterns, where `*`
matches anything. An entry is dropped if every value in at least one rule matches. It runs after variants,
and compares values before they are substituted.

```toml
[generators.bls]
# ...
exclude = [
  { version = "0-rescue-*" },
  { console = "graphics", mode = "debug" },
]
```

The first rule drops BLS rescue entries. The second drops only the one combination.

## `phases`

Phases run actions at fixed points in the boot. Each phase is a list, and each item names actions and can
add values for them.

| Phase     | Runs                                                       |
|-----------|------------------------------------------------------------|
| `early`   | Before drivers load.                                       |
| `startup` | After drivers, autoconfiguration and extractors, before the menu is built. |
| `late`    | After an entry is chosen, before its actions run.          |

```toml
[[phases.early]]
actions = ["splash"]

[[phases.startup]]
actions = ["announce"]
values.message = "drivers are loaded"
```

Items run in order, and an action that fails stops the boot with an error. Note the double brackets: each
item is one element of a list.

## Autoconfiguration

With `autoconfigure = true`, or `--autoconfigure`, Sprout looks at every filesystem it can read and adds
configuration for what it finds. With the option, the result is added to your file. With the flag, your file
is not read at all, which makes it a good way to get a machine booting before you've written one.

For each filesystem:

- **BLS**: if it has `\loader\loader.conf`, a `\loader\entries` directory with `.conf` files, or `.efi` files
  in `\EFI\Linux`, Sprout adds a `bls` generator and the action that boots its entries. These are named
  `auto-bls-<hash>`.
- **Linux**: if there was no BLS, kernels in `\boot` and `\` are found by name (`vmlinuz` or `Image`,
  optionally followed by `-` and a version), paired with an `initramfs`, `initrd` or `initrd.img` of the same
  suffix. The entries are named `auto-linux-<hash>` and titled "Boot Linux" plus the path. Command line
  options come from the value `linux-options`, which you can set in `[values]` and which otherwise is the
  placeholder `placeholder`.
- **Windows**: if `\EFI\Microsoft\Boot\bootmgfw.efi` exists, an entry "Boot Windows" named
  `auto-windows-<hash>` chainloads it.

`<hash>` is a short hash of the filesystem's device path, so two disks never collide. [Strict
mode](#strict-mode) narrows which filesystems the BLS part reads.

## Bootloader Specification support

Sprout implements the [Boot Loader Specification](https://uapi-group.org/specifications/specs/boot_loader_specification/)
to read the entries that distributions install, and sorts them as the specification says.

It reads Type #1 entries from `\loader\entries` and Type #2 unified kernel images (UKIs) from `\EFI\Linux`. A
UKI with several profiles is one entry per profile. Entries for another architecture than the machine's are
hidden. In a Type #1 entry Sprout uses the keys `title`, `version`, `machine-id`, `sort-key`, `linux`, `efi`,
`uki`, `initrd`, `options`, `devicetree`, `architecture` and `profile`.

The simplest way to get this is `autoconfigure = true`. To write it out by hand, you need a `bls` generator
and an action that boots what it makes:

```toml
[generators.bls]
bls.entry.title = "$title"
bls.entry.actions = ["boot-bls"]
bls.xbootldr = true

[actions.boot-bls]
chainload.path = "$entry-root\\$chainload"
chainload.options = ["$options"]
chainload.devicetree = "$entry-root\\$devicetree"
chainload.linux-initrd-chain = [
  "$entry-root\\$initrd-0",
  "$entry-root\\$initrd-1",
  "$entry-root\\$initrd-2",
  "$entry-root\\$initrd-3",
]
```

An entry can have up to 32 initrds, and Sprout warns when it has one that the chain doesn't list, because
the kernel likely can't then find its root. Autoconfiguration lists all 32. Unused slots are empty and
skipped, so listing more than you need costs nothing.

### BLS values

| Value                | Holds                                                                      |
|----------------------|----------------------------------------------------------------------------|
| `$title`             | The title, with the version added unless it already contains it.           |
| `$title-base`        | The title as written in the entry.                                         |
| `$version`           | The `version` field.                                                       |
| `$machine-id`        | The `machine-id` field.                                                    |
| `$chainload`         | The path of the `linux`, `efi` or `uki` file.                              |
| `$options`           | The `options` field, with the profile prepended for a UKI profile.         |
| `$cmdline`           | The command line embedded in a UKI.                                        |
| `$uname`             | The kernel version embedded in a UKI.                                      |
| `$devicetree`        | The `devicetree` path.                                                     |
| `$initrd-0` to `$initrd-31` | The initrds, in order. A slot with none is empty.                   |
| `$initrd`            | The last initrd.                                                           |
| `$entry-root`        | The device the entry's files are on. Empty for Sprout's own partition.     |

`$entry-root` matters once `xbootldr` is on, since an entry on the XBOOTLDR partition has its files there,
not next to Sprout. Always write BLS paths as `$entry-root\\$chainload`.

### Boot counting

A file named like `fedora+3.conf` or `fedora+3.efi` has three tries. Each boot renames it, say to
`fedora+2-1.conf`, and tells the system the new path in `LoaderBootCountPath` so `systemd-bless-boot` can
remove the counter once the boot works.

Entries with no tries left sort last and are never the default, but can still be booted by hand. If an entry
fails to start after it used a try, and it had tries left, the machine resets so the next boot can use the
next try or pick another entry. This is the `reboot-on-error` setting of `loader.conf`. Without boot counting
a failure to start goes back to the firmware.

### `loader.conf`

Sprout reads `\loader\loader.conf` from the partition it was loaded from, as systemd-boot does. It is
measured into the TPM, since it changes how Sprout boots.

| Key               | Value                                                              |
|-------------------|--------------------------------------------------------------------|
| `default`         | A pattern for the id of the default entry, or `@saved`.            |
| `preferred`       | Like `default`, but entries with no boot counter tries are skipped. |
| `timeout`         | Seconds, `menu-hidden`, `menu-disabled` or `menu-force`.           |
| `reboot-on-error` | `auto` (the default), `yes` or `no`.                               |

The id of an entry is its file name without the boot counter, like `fedora.conf` or `fedora.efi`. Patterns
ignore case and can use `*`, `?` and `[a-z]`. With `@saved` the entry booted last is the default.

`reboot-on-error = yes` resets after every failed start, which can loop forever. `auto` only resets when a
try was used up and some were left. Other keys are ignored with a warning.

A hidden menu, from a timeout of zero or `menu-hidden`, still opens when a key is pressed. `menu-disabled`
does not.

### The bootloader interface

Sprout uses the same firmware variables as systemd-boot, so `bootctl`, `systemctl reboot --boot-loader-entry`
and `systemd-bless-boot` work with it. It publishes `LoaderEntries`, `LoaderEntrySelected`,
`LoaderBootCountPath`, `LoaderFeatures` and `LoaderInfo`, and reads `LoaderEntryDefault`,
`LoaderEntryPreferred`, `LoaderEntryOneShot`, `LoaderEntryLastBooted`, `LoaderConfigTimeout` and
`LoaderConfigTimeoutOneShot`.

What `bootctl` sets outranks `sprout.toml`, which outranks `loader.conf`. The default entry is the first of
these that matches an entry:

1. `LoaderEntryPreferred`, then `preferred` in `loader.conf`
2. `LoaderEntryDefault`
3. `default-entry` in `sprout.toml`
4. `default` in `loader.conf`

The menu timeout is the first of these that is set:

1. the one-shot timeout, `LoaderConfigTimeoutOneShot`
2. `--menu-timeout`
3. `LoaderConfigTimeout`
4. `menu-timeout` in `sprout.toml`
5. `timeout` in `loader.conf`

A one-shot entry (`LoaderEntryOneShot`) is booted at once and without the menu, unless strict mode is on.

## Strict mode

By default Sprout departs from systemd-boot in a few places, where it is friendlier or safer for a
bootloader that reads the same files. Each departure is logged when it applies. `--bls-strict-mode`, or
`bls-strict-mode = true` under `[options]`, turns all of them off.

| Behavior                                          | By default                                              | In strict mode                                          |
|---------------------------------------------------|---------------------------------------------------------|---------------------------------------------------------|
| One-shot entry (`LoaderEntryOneShot`)             | Booted at once, without the menu.                       | Only the default for this boot. The menu and its timeout still apply. |
| Menu timeout that nothing sets                    | The menu is shown for 10 seconds.                       | The menu is hidden.                                     |
| Default entry with no boot counter tries          | Skipped for another entry.                              | Used, as `default` ignores the tries.                   |
| Entry with more than one of `linux`, `efi`, `uki` | Boots the first of them.                                | Hidden.                                                 |
| Entry whose file does not exist                   | Shown, and it fails when booted.                        | Hidden.                                                 |
| UKI without a name                                | Named after its file.                                   | Hidden.                                                 |
| Name and version of a UKI                         | Name is `PRETTY_NAME` or `ID`. Version is `IMAGE_VERSION`, `VERSION_ID`, `BUILD_ID`, then `.uname`. | What systemd-boot uses. Name is `PRETTY_NAME`, `IMAGE_ID`, `NAME` or `ID`. Version is `IMAGE_VERSION`, `VERSION`, `VERSION_ID` or `BUILD_ID`. |
| Boot counter of a plain `efi` entry               | Counted.                                                | Not counted, but its tries still make it bad.           |
| Extended boot loader partition                    | Read when `xbootldr` is set on the generator.           | Always read.                                            |
| Autoconfiguration                                 | Reads BLS entries from every disk, one generator each.  | Reads BLS entries from Sprout's partition and the XBOOTLDR of its disk, as one generator. Windows and Linux entries are found as before. |
| Boot entry that returns                           | Sprout exits to the firmware.                           | The menu is shown again.                                |
| Devicetree that can't be installed                | Warns and boots without it.                             | The entry fails.                                        |

In strict mode the actions of a hand-written generator have to use `$entry-root` for the files of an entry,
as entries can come from the extended boot loader partition.
