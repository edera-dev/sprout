#!/bin/sh
set -e

. /build/src/kernel.buildenv

[ -f "arch/x86/boot/bzImage" ] && cp "arch/x86/boot/bzImage" kernel.image
[ -f "arch/arm64/boot/Image.gz" ] && gzip -d <"arch/arm64/boot/Image.gz" >kernel.image

make CROSS_COMPILE="${MAYBE_CROSS_COMPILE}" ARCH="${TARGET_KARCH}" INSTALL_MOD_PATH="/build/install" modules_install
cd /build/install
tar czpf /build/src/kernel.modules.tgz .
