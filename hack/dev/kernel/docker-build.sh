#!/bin/sh
set -e

TARGET_KARCH=""
TARGET_SARCH=""

MAYBE_CROSS_COMPILE=""

CURRENT_SARCH="$(uname -m)"

[ "${CURRENT_SARCH}" = "amd64" ] && CURRENT_SARCH="x86_64"
[ "${CURRENT_SARCH}" = "arm64" ] && CURRENT_SARCH="aarch64"

if [ "${TARGETPLATFORM}" = "linux/aarch64" ] || [ "${TARGETPLATFORM}" = "linux/arm64" ]; then
	TARGET_KARCH="arm64"
	TARGET_SARCH="aarch64"
	if [ "${CURRENT_SARCH}" != "${TARGET_SARCH}" ]; then
		MAYBE_CROSS_COMPILE="aarch64-linux-gnu-"
	fi
elif [ "${TARGETPLATFORM}" = "linux/x86_64" ] || [ "${TARGETPLATFORM}" = "linux/amd64" ]; then
	TARGET_KARCH="x86_64"
	TARGET_SARCH="x86_64"
	if [ "${CURRENT_SARCH}" != "${TARGET_SARCH}" ]; then
		MAYBE_CROSS_COMPILE="x86_64-linux-gnu-"
	fi
else
	echo "Unknown platform: ${TARGETPLATFORM}" >/dev/stderr
	exit 1
fi

make CROSS_COMPILE="${MAYBE_CROSS_COMPILE}" ARCH="${TARGET_KARCH}" defconfig
make CROSS_COMPILE="${MAYBE_CROSS_COMPILE}" ARCH="${TARGET_KARCH}" mod2yesconfig

./scripts/config -e DRM_VIRTIO_GPU
./scripts/config -e FRAMEBUFFER_CONSOLE
./scripts/config -e FRAMEBUFFER_CONSOLE_DETECT_PRIMARY

make "-j$(nproc)" CROSS_COMPILE="${MAYBE_CROSS_COMPILE}" ARCH="${TARGET_KARCH}"

[ -f "arch/x86/boot/bzImage" ] && cp "arch/x86/boot/bzImage" kernel.image
[ -f "arch/arm64/boot/Image.gz" ] && gzip -d < "arch/arm64/boot/Image.gz" > kernel.image
exit 0
