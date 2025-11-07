# Setup Sprout for openSUSE with Secure Boot

**NOTE:** This guide may not function as written if the system validates hashes.
If your system validates hashes in the shim, you will need to use MokManager to enroll the hashes
of every EFI file involved, such as Sprout and any EFI drivers.

## Prerequisites

- Modern openSUSE release: tested on openSUSE Tumbleweed ARM64
- EFI System Partition mounted on `/boot/efi` (the default)
- You will need the following packages installed: `openssl`, `shim`, `mokutil`, `sbsigntools`

## Step 1: Generate and Install Secure Boot Key

```bash
# Create a directory to store the Secure Boot MOK key and certificates.
$ mkdir -p /etc/sprout/secure-boot
# Change to the created directory.
$ cd /etc/sprout/secure-boot
# Generate a MOK key and certificate.
$ openssl req \
    -newkey rsa:4096 -nodes -keyout mok.key \
    -new -x509 -sha256 -days 3650 -subj '/CN=Sprout Secure Boot/' \
    -out mok.crt
# Generate a DER encoded certificate for enrollment.
$ openssl x509 -outform DER -in mok.crt -out mok.cer
# Import the certificate into the Secure Boot environment.
# This will ask you to make a password that will be used during enrollment.
$ mokutil --import mok.cer
# Reboot your machine.
# During boot, MOK enrollment should appear. If it doesn't, ensure you are booting into the shim.
# Press any key to begin MOK management. Select "Enroll MOK".
# Select "View key 0", and ensure the subject says "CN=Sprout Secure Boot".
# If the subject does not match, something has gone wrong with MOK enrollment.
# Press Enter to continue, then select the "Continue" option.
# When it asks to enroll the key, select the "Yes" option.
# Enter the password that you created during the mokutil --import step.
# Select "Reboot" to boot back into your Operating System.
```

## Step 2: Prepare the Secure Boot Environment

```bash
# Create a directory for Sprout EFI artifacts.
$ mkdir -p /boot/efi/EFI/sprout

# For x86_64, copy the following artifacts to the Sprout EFI directory.
$ cp /usr/share/efi/x86_64/shim.efi /boot/efi/EFI/sprout/shim.efi
$ cp /usr/share/efi/x86_64/MokManager.efi /boot/efi/EFI/sprout/MokManager.efi
$ cp /usr/share/efi/x86_64/fallback.efi /boot/efi/EFI/sprout/fallback.efi

# For aarch64, copy the following artifacts to the Sprout EFI directory.
$ cp /usr/share/efi/aarch64/shim.efi /boot/efi/EFI/sprout/shim.efi
$ cp /usr/share/efi/aarch64/MokManager.efi /boot/efi/EFI/sprout/MokManager.efi
$ cp /usr/share/efi/aarch64/fallback.efi /boot/efi/EFI/sprout/fallback.efi
```

## Step 3: Install Unsigned Sprout

Download the latest sprout.efi release from the [GitHub releases page](https://github.com/edera-dev/sprout/releases).
For x86_64 systems, download the `sprout-x86_64.efi` file, and for ARM64 systems, download the `sprout-aarch64.efi`
file.
Copy the downloaded `sprout.efi` file to `/boot/efi/EFI/sprout/sprout.unsigned.efi` on your EFI System Partition.

## Step 4: Sign Sprout for Secure Boot

```bash
# Sign the unsigned Sprout artifact and name it grub.efi which is what the shim will call.
$ sbsign \
    --key /etc/sprout/secure-boot/mok.key \
    --cert /etc/sprout/secure-boot/mok.crt \
    --output /boot/efi/EFI/sprout/grub.efi \
    /boot/efi/EFI/sprout/sprout.unsigned.efi
```

## Step 5: Install and Sign EFI Drivers

You will need a filesystem EFI driver if `/boot` is not FAT32 or ExFAT.
If `/boot` is FAT32 or ExFAT, you can skip this step.

Most Debian systems use an ext4 filesystem for `/boot`.
You can download an EFI filesystem driver from [EfiFs releases](https://github.com/pbatard/EfiFs/releases).
For ext4, download the `ext2` file for your platform. It should work for ext4 filesystems too.

If you have an EFI driver, copy the driver to `/boot/efi/EFI/sprout/DRIVER_NAME.unsigned.efi` for signing.

For example, the `ext4` driver, copy the `ext4.efi` file to `/boot/efi/EFI/sprout/ext4.unsigned.efi`.

Then sign the driver with the Sprout Secure Boot key:

```bash
# Sign the ext4 driver at ext4.unsigned.efi, placing it at ext4.efi, which will be used in the configuration.
$ sbsign \
    --key /etc/sprout/secure-boot/mok.key \
    --cert /etc/sprout/secure-boot/mok.crt \
    --output /boot/efi/EFI/sprout/ext4.efi \
    /boot/efi/EFI/sprout/ext4.unsigned.efi
```

## Step 6: Create Sprout Configuration

Write the following to the file `/boot/efi/sprout.toml`:

```toml
# sprout configuration: version 1
version = 1

# global values.
[values]
# your linux kernel command line.
linux-options = "root=UUID=MY_ROOT_UUID"

# load an ext4 EFI driver.
# skip this if you do not have a filesystem driver.
# if your filesystem driver is not named ext4, change accordingly.
[drivers.ext4]
path = "\\EFI\\sprout\\ext4.efi"

# global options.
[options]
# enable autoconfiguration by detecting bls enabled filesystems
# or linux kernels and generating boot entries for them.
autoconfigure = true
```

Ensure you add the signed driver paths to the configuration, not the unsigned ones.
If you do not have any drivers, exclude the drivers section entirely.

## Step 7: Configure Sprout Boot Entry

In the following commands, replace /dev/BLOCK_DEVICE with the device that houses your GPT partition table,
and PARTITION_NUMBER with the partition number of the EFI System Partition. For example, if your EFI System Partition is
`/dev/sda1`, the BLOCK_DEVICE would be `/dev/sda` and the PARTITION_NUMBER would be `1`

```bash
# Run this command to add Sprout as the default boot entry.
$ efibootmgr -d /dev/BLOCK_DEVICE -p PARTITION_NUMBER -c -L 'Sprout' -l '\EFI\sprout\shim.efi'
```

Reboot your machine and it should boot into Sprout.
If Sprout fails to boot, it should boot into the original bootloader.
