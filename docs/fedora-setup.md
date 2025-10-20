# Setup Sprout on Fedora

## Prerequisites

- Modern Fedora release: tested on Fedora Workstation 42 and 43
- EFI System Partition mounted on `/boot/efi` (the default)
- ext4 or FAT32/exFAT formatted `/boot` partition

## Step 1: Base Installation

Download the latest sprout.efi release from the [GitHub releases page](https://github.com/edera-dev/sprout/releases).
For x86_64 systems, download the `sprout-x86_64.efi` file, and for ARM systems, download the `sprout-aarch64.efi` file.
Copy the downloaded `sprout.efi` file to `/boot/efi/EFI/BOOT/sprout.efi` on your EFI System Partition.

Additionally, you will want to install the `edk2-ext4` package, which provides the ext4 filesystem support for Sprout.

```bash
# Install the edk2-ext4 package which provides ext4 support for Sprout.
$ sudo dnf install edk2-ext4
# Create a directory for sprout drivers.
$ sudo mkdir -p /boot/efi/sprout/drivers
# For x86_64 systems, copy the ext4x64.efi driver to the sprout drivers directory.
$ sudo cp /usr/share/edk2/drivers/ext4x64.efi /boot/efi/sprout/drivers/ext4.efi
# For ARM64 systems, copy the ext4aa64.efi driver to the sprout drivers directory.
$ sudo cp /usr/share/edk2/drivers/ext4aa64.efi /boot/efi/sprout/drivers/ext4.efi
```

## Step 2: Configure Sprout

Since Fedora uses the BLS specification, you can use the `bls` generator to autoconfigure Sprout for Fedora.

Write the following file to `/boot/efi/sprout.toml`:

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

## Step 3, Option 1: Configure GRUB to load Sprout (recommended)

You can configure GRUB to add a boot entry for Sprout, so you can continue to use GRUB without interruption.

GRUB needs to be configured with the chainloader module to load Sprout.

### x86_64

```bash
# Install x86_64 GRUB modules.
$ sudo dnf install grub2-efi-x64-modules
# Copy x86_64 GRUB modules to /boot/grub2 for use by GRUB if it isn't installed already.
$ [ ! -d /boot/grub2/x86_64-efi ] && sudo cp -r /usr/lib/grub/x86_64-efi /boot/grub2/x86_64-efi
```

### ARM64

```bash
# Install ARM64 GRUB modules.
$ sudo dnf install grub2-efi-aa64-modules
# Copy ARM64 GRUB modules to /boot/grub2 for use by GRUB if it isn't installed already.
$ [ ! -d /boot/grub2/arm64-efi ] && sudo cp -r /usr/lib/grub/arm64-efi /boot/grub2/x86_64-efi
```

You will need to find the UUID of your EFI System Partition. You can do this by running the following command:
```bash
$ grep "/boot/efi" /etc/fstab | awk '{print $1}' | awk -F '=' '{print $2}'
SAMPLE-VALUE
```

The GRUB configuration for Sprout is as follows, replace `SAMPLE-VALUE` with the UUID of your EFI System Partition:

```grub
menuentry 'Sprout' $menuentry_id_option 'sprout' {
        insmod part_gpt
        insmod fat
        insmod chain
        search --no-floppy --fs-uuid --set=root SAMPLE-VALUE
        chainloader /EFI/BOOT/sprout.efi
}
```

You can append this to `/etc/grub.d/40_custom` and run the following command to update your configuration:
```bash
$ sudo grub2-mkconfig -o /boot/grub2/grub.cfg
```

To update your GRUB configuration.

Make sure to update your GRUB environment to show the menu:

```bash
$ sudo grub2-editenv - set menu_auto_hide=0
```

You may now reboot your system and select Sprout from the GRUB menu.

## Step 3, Option 2: Configure your EFI firmware for Sprout

You can configure your EFI boot menu to show Sprout as an option.

To do so, please find the partition device of your EFI System Partition and run the following:

```bash
$ sudo efibootmgr -d /dev/esp_partition_here -C -L 'Sprout' -l '\EFI\BOOT\sprout.efi'
```

This will add a new entry to your EFI boot menu called `Sprout` that will boot Sprout with your configuration.

Now if you boot into your UEFI firmware, you should see Sprout as an option to boot.
