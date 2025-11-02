# Ubuntu Secure Boot Setup

## Generate and Install Secure Boot Key

```bash
# Create a directory to store the Secure Boot MOK key and certificates.
mkdir -p /etc/sprout/secure-boot
# Change to the created directory.
cd /etc/sprout/secure-boot
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

## Prepare Secure Boot Environment

```bash
# Create a directory for Sprout EFI artifacts.
$ mkdir -p /boot/efi/EFI/sprout

# For x86_64, copy the following artifacts to the Sprout EFI directory.
$ cp /usr/lib/shim/shimx64.efi.signed /boot/efi/EFI/sprout/shimx64.efi
$ cp /usr/lib/shim/mmx64.efi /boot/efi/EFI/sprout/mmx64.efi
$ cp /usr/lib/shim/fbx64.efi /boot/efi/EFI/sprout/fbx64.efi

# For aarch64, copy the following artifacts to the Sprout EFI directory.
$ cp /usr/lib/shim/shimaa64.efi.signed /boot/efi/EFI/sprout/shimaa64.efi
$ cp /usr/lib/shim/mmaa64.efi /boot/efi/EFI/sprout/mmaa64.efi
$ cp /usr/lib/shim/fbaa64.efi /boot/efi/EFI/sprout/fbaa64.efi
```

## Install Unsigned Sprout

Download the latest sprout.efi release from the [GitHub releases page](https://github.com/edera-dev/sprout/releases).
For x86_64 systems, download the `sprout-x86_64.efi` file, and for ARM64 systems, download the `sprout-aarch64.efi` file.
Copy the downloaded `sprout.efi` file to `/boot/efi/EFI/sprout/sprout.unsigned.efi` on your EFI System Partition.

## Sign Sprout for Secure Boot

```bash
# For x86_64, sign the unsigned Sprout artifact and name it grubaa64.efi which is what the shim will call.
$ sbsign \
    --key /etc/sprout/secure-boot/mok.key \
    --cert /etc/sprout/secure-boot/mok.crt \
    --output /boot/efi/EFI/sprout/grubx64.efi \
    /boot/efi/EFI/sprout/sprout.unsigned.efi

# For aarch64, sign the unsigned Sprout artifact and name it grubaa64.efi which is what the shim will call.
$ sbsign \
    --key /etc/sprout/secure-boot/mok.key \
    --cert /etc/sprout/secure-boot/mok.crt \
    --output /boot/efi/EFI/sprout/grubaa64.efi \
    /boot/efi/EFI/sprout/sprout.unsigned.efi
```

## Sign EFI Drivers

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

## Create Sprout Configuration

Write the following to the file `/boot/efi/sprout.toml`:

```toml
# sprout configuration: version 1
version = 1

# global values.
[values]
# your linux kernel command line.
linux-options = "root=UUID=MY_ROOT_UUID"

# load an ext4 EFI driver.
[drivers.ext4]
path = "\\EFI\\sprout\\ext4.efi"

# global options.
[options]
# enable autoconfiguration by detecting bls enabled
# filesystems and generating boot entries for them.
autoconfigure = true
```

Ensure you add the signed driver paths to the configuration, not the unsigned ones.
If you do not have any drivers, exclude the drivers section entirely.

## Configure Sprout Boot Entry

```bash
# In the following commands, replace /dev/ESP_PARTITION with the actual path to the ESP partition block device.

# For x86_64, run this command to add Sprout as the default boot entry.
$ efibootmgr -d /dev/ESP_PARTITION -c -L 'Sprout' -l '\EFI\sprout\shimx64.efi'

# For aarch64, run this command to add Sprout as the default boot entry.
$ efibootmgr -d /dev/ESP_PARTITION -c -L 'Sprout' -l '\EFI\sprout\shimaa64.efi'
```
