#!/bin/sh
set -e

cd "$(dirname "${0}")/../.." || exit 1

. "hack/common.sh"

if [ "${SKIP_BUILD}" != "1" ]; then
	./hack/dev/build.sh "${TARGET_ARCH}" "${RUST_PROFILE}"
fi

clear

set --
if [ "${TARGET_ARCH}" = "x86_64" ]; then
	set -- "${@}" qemu-system-x86_64 -M q35
elif [ "${TARGET_ARCH}" = "aarch64" ]; then
	set -- "${@}" qemu-system-aarch64 -M virt -cpu cortex-a57
fi

if [ -n "${QEMU_ACCEL}" ]; then
	set -- "${@}" "-accel" "kvm"
fi

if [ "${QEMU_GDB}" = "1" ]; then
	set -- "${@}" "-s"
fi

if [ "${QEMU_GDB_WAIT}" = "1" ]; then
	set -- "${@}" "-S"
fi

set -- "${@}" -smp 2 -m 4096

if [ "${NO_GRAPHICAL_BOOT}" = "1" ]; then
	set -- "${@}" -nographic
else
	if [ "${QEMU_LEGACY_SERIAL}" = "1" ]; then
		set -- "${@}" -serial stdio
	else
		set -- "${@}" \
			-device virtio-serial-pci,id=vs0 \
			-chardev stdio,id=stdio0 \
			-device virtconsole,chardev=stdio0,id=console0
	fi

	if [ "${QEMU_LEGACY_VGA}" = "1" ]; then
		set -- "${@}" -vga std
	else
		set -- "${@}" \
			-vga none \
			-device "virtio-gpu,edid=on,xres=1024,yres=768"
	fi
fi

rm -f "${FINAL_DIR}/ovmf-boot.fd"
cp "${FINAL_DIR}/ovmf.fd" "${FINAL_DIR}/ovmf-boot.fd"
if [ "${TARGET_ARCH}" = "aarch64" ]; then
	dd if=/dev/zero of="${FINAL_DIR}/ovmf-boot.fd" bs=1 count=1 seek=67108863 >/dev/null 2>&1
fi
# shellcheck disable=SC2086
set -- "${@}" \
	-drive "if=pflash,file=${FINAL_DIR}/ovmf-boot.fd,format=raw,readonly=on" \
	-device nvme,drive=disk1,serial=cafebabe

if [ "${DISK_BOOT}" = "1" ]; then
	set -- "${@}" \
		-drive "if=none,file=${FINAL_DIR}/sprout.img,format=raw,id=disk1,readonly=on"
else
	set -- "${@}" \
		-drive "if=none,file=fat:rw:${FINAL_DIR}/efi,format=raw,id=disk1"
fi

set -- "${@}" -name "sprout ${TARGET_ARCH}"

exec "${@}"
