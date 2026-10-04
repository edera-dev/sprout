#!/bin/sh
set -e

cd "$(dirname "${0}")/../.." || exit 1

. "hack/common.sh"

if [ "${SKIP_BUILD}" != "1" ]; then
	./hack/dev/build.sh "${TARGET_ARCH}" "${RUST_PROFILE}"
fi

[ -t 1 ] && clear

set --
if [ "${TARGET_ARCH}" = "x86_64" ]; then
	set -- "${@}" qemu-system-x86_64 -M q35 -cpu SandyBridge,vmx=on
elif [ "${TARGET_ARCH}" = "aarch64" ]; then
	# Hardware acceleration on aarch64 only supports the host CPU model.
	if [ -n "${QEMU_ACCEL}" ] && [ "${QEMU_ACCEL}" != "tcg" ]; then
		set -- "${@}" qemu-system-aarch64 -M virt -cpu host
	else
		set -- "${@}" qemu-system-aarch64 -M virt -cpu cortex-a57
	fi
fi

if [ -n "${QEMU_ACCEL}" ]; then
	set -- "${@}" "-accel" "${QEMU_ACCEL}"
fi

if [ "${QEMU_GDB}" = "1" ]; then
	set -- "${@}" "-s"
fi

if [ "${QEMU_GDB_WAIT}" = "1" ]; then
	set -- "${@}" "-S"
fi

set -- "${@}" -nodefaults -smp 2 -m 4096

if [ "${NO_GRAPHICAL}" = "1" ]; then
	set -- "${@}" -nographic
else
	if [ "${GRAPHICAL_ONLY}" != "1" ] && [ "${QEMU_HEADLESS}" != "1" ]; then
		if [ "${QEMU_LEGACY_SERIAL}" = "1" ]; then
			set -- "${@}" -serial stdio
		else
			set -- "${@}" \
				-device 'virtio-serial-pci,id=vs0' \
				-chardev 'stdio,id=stdio0,signal=off' \
				-device 'virtconsole,chardev=stdio0,id=console0,name=alpine'
		fi
	fi

	# Headless keeps the virtual GPU, so screenshots still work, but shows no window. The firmware
	# console and the Linux console go to files, as there is no terminal to attach to.
	if [ "${QEMU_HEADLESS}" = "1" ]; then
		set -- "${@}" \
			-display none \
			-serial "file:${QEMU_SERIAL_FILE:-/dev/null}" \
			-device 'virtio-serial-pci,id=vs0' \
			-chardev "file,id=file0,path=${QEMU_CONSOLE_FILE:-/dev/null}" \
			-device 'virtconsole,chardev=file0,id=console0,name=alpine'
	fi

	if [ "${QEMU_LEGACY_VGA}" = "1" ]; then
		set -- "${@}" -vga std
	else
		set -- "${@}" \
			-vga none \
			-device "virtio-gpu,edid=on,xres=1024,yres=768"
	fi
fi

if [ "${NO_INPUT}" != "1" ]; then
	set -- "${@}" \
		-device qemu-xhci \
		-device usb-kbd \
		-device usb-mouse
fi

if [ "${NO_NETWORK}" != "1" ]; then
	set -- "${@}" \
		-netdev 'user,id=network0' \
		-device 'virtio-net-pci,netdev=network0'
fi

# The firmware in the dev image hangs before it starts Sprout when accelerated on Apple Silicon,
# so use the firmware that comes with Homebrew's QEMU there. QEMU_FIRMWARE overrides the choice.
if [ -z "${QEMU_FIRMWARE}" ] && [ "${TARGET_ARCH}" = "aarch64" ] && [ "${QEMU_ACCEL}" = "hvf" ] &&
	command -v brew >/dev/null 2>&1; then
	BREW_FIRMWARE="$(brew --prefix qemu 2>/dev/null)/share/qemu/edk2-aarch64-code.fd"
	[ -f "${BREW_FIRMWARE}" ] && QEMU_FIRMWARE="${BREW_FIRMWARE}"
fi

rm -f "${FINAL_DIR}/ovmf-boot.fd"
cp "${QEMU_FIRMWARE:-${FINAL_DIR}/ovmf.fd}" "${FINAL_DIR}/ovmf-boot.fd"
if [ "${TARGET_ARCH}" = "aarch64" ]; then
	dd if=/dev/zero of="${FINAL_DIR}/ovmf-boot.fd" bs=1 count=1 seek=67108863 >/dev/null 2>&1
fi
# shellcheck disable=SC2086
set -- "${@}" \
	-drive "if=pflash,file=${FINAL_DIR}/ovmf-boot.fd,format=raw,readonly=on" \
	-device 'nvme,drive=disk1,serial=cafebabe'

set -- "${@}" \
	-drive "if=none,file=${FINAL_DIR}/sprout.img,format=raw,id=disk1,readonly=on"

set -- "${@}" -name "sprout ${TARGET_ARCH}"

if [ -n "${QEMU_QMP}" ]; then
	set -- "${@}" -qmp "unix:${QEMU_QMP},server=on,wait=off"
fi

# shellcheck disable=SC2086
set -- "${@}" ${QEMU_EXTRA_ARGS}

exec "${@}"
