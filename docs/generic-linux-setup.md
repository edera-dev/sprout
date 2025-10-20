# Setup Sprout to boot Linux

## Prerequisites

- EFI System Partition mounted on a known path
- Linux kernel installed with an optional initramfs
- Linux kernel must support the EFI stub (most distro kernels)

## Step 1: Base Installation

First, identify the path to your EFI System Partition. On most systems, this is `/boot/efi`.

Download the latest sprout.efi release from the [GitHub releases page](https://github.com/edera-dev/sprout/releases).
For x86_64 systems, download the `sprout-x86_64.efi` file, and for ARM systems, download the `sprout-aarch64.efi` file.
Copy the downloaded `sprout.efi` file to `/EFI/BOOT/sprout.efi` on your EFI System Partition.

## Step 2: Copy kernel and optional initramfs

Copy the Linux kernel to `/vmlinuz-sprout` on your EFI System Partition.
If needed, copy the initramfs to `/initramfs-sprout` on your EFI System Partition.

## Step 3: Configure Sprout

Write the following file to `/sprout.toml` on your EFI System Partition,
paying attention to place the correct values:

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
chainload.path = "\\vmlinuz-sprout"
chainload.options = ["root=/dev/sda1", "my-kernel-option"]
chainload.linux-initrd = "\\initramfs-sprout"
```

You can specify any kernel command line options you want on the chainload options line.
They will be concatenated by a space and passed to the kernel.

## Step 4: Configure EFI firmware to boot Sprout

Since Sprout is still experimental, the following commands will add a boot entry to your EFI firmware for sprout but
intentionally do not set it as the default boot entry.

To add the entry, please find the partition device of your EFI System Partition and run the following:

```bash
$ sudo efibootmgr -d /dev/esp_partition_here -C -L 'Sprout' -l '\EFI\BOOT\sprout.efi'
```

This will add a new entry to your EFI boot menu called `Sprout` that will boot Sprout with your configuration.
Now if you boot into your UEFI firmware, you should see Sprout as an option to boot.
