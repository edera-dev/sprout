# Setup Sprout to boot Windows

## Prerequisites

- Secure Boot disabled
- UEFI Windows installation

## Step 1: Base Installation

First, mount the EFI System Partition on your Windows installation:

In an administrator command prompt, run:

```batch
> mountvol X: /s
```

This will mount the EFI System Partition to the drive letter `X:`.

Please note that Windows Explorer will not let you see the drive letter `X:` where the ESP is mounted.
You will need to use the command prompt or PowerShell to access the ESP.
Standard editors can, however, be used to edit files on the ESP.

Download the latest sprout.efi release from the [GitHub releases page](https://github.com/edera-dev/sprout/releases).
For x86_64 systems, download the `sprout-x86_64.efi` file, and for ARM systems, download the `sprout-aarch64.efi` file.
Copy the downloaded `sprout.efi` file to `X:\EFI\BOOT\sprout.efi` on your EFI System Partition.

## Step 3: Configure Sprout

Write the following file to `X:\sprout.toml`:

```toml
# sprout configuration: version 1
version = 1

# add a boot entry for booting Windows
# which will run the boot-windows action.
[entries.windows]
title = "Windows"
actions = ["boot-windows"]

# use the chainload action to boot the Windows bootloader.
[actions.boot-windows]
chainload.path = "\\EFI\\Microsoft\\Boot\\bootmgfw.efi"
```

## Step 4: Configure EFI Firmware to boot Sprout

It is not trivial to add an EFI boot entry inside Windows.
However, most firmware lets you load arbitrary EFI files from the firmware settings.

You can boot `\EFI\BOOT\sprout.efi` from firmware to boot Sprout.
