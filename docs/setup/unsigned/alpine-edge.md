# Setup Sprout for Alpine Edge without Secure Boot

## Prerequisites

- Alpine Edge
- EFI System Partition mounted on `/boot/efi` (the default)
- ext4 or FAT32/exFAT formatted `/boot` partition

## Step 1: Base Installation

Download the latest sprout.efi release from the [GitHub releases page](https://github.com/edera-dev/sprout/releases).
For x86_64 systems, download the `sprout-x86_64.efi` file, and for ARM systems, download the `sprout-aarch64.efi` file.
Copy the downloaded `sprout.efi` file to `/boot/efi/EFI/boot/sprout.efi` on your EFI System Partition.

Additionally, you will want to install the `efifs` package, which provides the filesystem support for Sprout.

```bash
# Install the efifs package which provides filesystem support for Sprout.
$ apk install efifs
```

## Step 2: Configure Sprout

Since Alpine uses standard image paths based on the `linux` package installed, it's quite easy to configure Sprout
to boot Alpine.

Write the following file to `/boot/efi/sprout.toml`:

```toml
# sprout configuration: version 1
version = 1

# load an EFI driver for ext2/ext3/ext4.
[drivers.ext2]
path = "\\EFI\\efifs\\ext2.efi"

# extract the full path of the first filesystem
# that contains \boot\vmlinuz-stable as a file
# into the value called "root"
[extractors.root.filesystem-device-match]
has-item = "\\boot\\vmlinuz-stable"

# add a boot entry for booting linux
# which will run the boot-linux action.
[entries.boot-linux-stable]
title = "Boot Linux Stable"
actions = ["boot-linux-stable"]

# use the chainload action to boot linux-stable via the efi stub.
# the options below are passed to the efi stub as the
# kernel command line. the initrd is loaded using the efi stub
# initrd loader mechanism.
[actions.boot-linux-stable]
chainload.path = "$root\\boot\\vmlinuz-stable"
chainload.options = ["root=/dev/sda1", "my-kernel-option"]
chainload.linux-initrd = "$root\\boot\\initramfs-stable"
```

You can replace `vmlinuz-stable` and `initramfs-stable` with the actual
files for the `linux` package you have installed. For example, for `linux-lts` it is `vmlinuz-lts` and `initramfs-lts`.

## Step 3, Option 1: Configure GRUB to load Sprout (recommended)

You can configure GRUB to add a boot entry for Sprout, so you can continue to use GRUB without interruption.

GRUB needs to be configured with the chainloader module to load Sprout.

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
        chainloader /EFI/boot/sprout.efi
}
```

You can append this to `/etc/grub.d/40_custom` and run the following command to update your configuration:
```bash
$ update-grub
```

To update your GRUB configuration.

You may now reboot your system and select Sprout from the GRUB menu.

## Step 3, Option 2: Configure your EFI firmware for Sprout

You can configure your EFI boot menu to show Sprout as an option.

You will need to install the `efibootmgr` package:

```
$ apk add efibootmgr
```

Once `efibootmgr` is installed, find the partition device of your EFI System Partition and run the following:

```bash
$ efibootmgr -d /dev/esp_partition_here -C -L 'Sprout' -l '\EFI\boot\sprout.efi'
```

This will add a new entry to your EFI boot menu called `Sprout` that will boot Sprout with your configuration.

Now if you boot into your UEFI firmware, you should see Sprout as an option to boot.
