#!/bin/sh
set -e

cd "$(dirname "${0}")/.." || exit 1

. "hack/common.sh"

EFI_NAME="BOOTX64"
if [ "${TARGET_ARCH}" = "aarch64" ]; then
	EFI_NAME="BOOTAA64"
fi

if [ -z "${SPROUT_CONFIG_NAME}" ]; then
	SPROUT_CONFIG_NAME="kernel"
fi

echo "[build] ${TARGET_ARCH} ${RUST_PROFILE}"

if ! command -v docker >/dev/null 2>&1; then
	echo "ERROR: docker is required to build sprout." >/dev/stderr
	exit 1
fi

export DOCKER_CLI_HINTS="0"

if [ "${SKIP_CLEANUP}" != 1 ]; then
	rm -rf "${FINAL_DIR}"
fi
mkdir -p "${FINAL_DIR}"

if [ "${SKIP_KERNEL_BUILD}" != "1" ] || [ "${SKIP_VM_BUILD}" != "1" ] || [ "${SKIP_SPROUT_BUILD}" != "1" ]; then
	docker build -t "${DOCKER_PREFIX}/sprout-utils-copy-direct:${DOCKER_TAG}" -f hack/utils/Dockerfile.copy-direct hack
fi

copy_from_image_direct() {
	IMAGE="${1}"
	SOURCE="${2}"
	TARGET="${3}"

	docker run --rm -i \
		--mount="type=image,source=${IMAGE},target=/image" \
		"${DOCKER_PREFIX}/sprout-utils-copy-direct:${DOCKER_TAG}" cat "/image/${SOURCE}" >"${TARGET}" 2>/dev/null
}

copy_from_image_polyfill() {
	IMAGE="${1}"
	SOURCE="${2}"
	TARGET="${3}"

	docker build -t "${IMAGE}-copy-polyfill:${DOCKER_TAG}" --build-arg "TARGET_IMAGE=${IMAGE}:${DOCKER_TAG}" \
		-f hack/utils/Dockerfile.copy-polyfill hack
	# note: the -w '//' is a workaround for Git Bash where / is magically rewritten.
	docker run --rm -i -w '//' "${IMAGE}-copy-polyfill:${DOCKER_TAG}" cat "image/${SOURCE}" >"${TARGET}"
}

copy_from_image() {
	if ! copy_from_image_direct "${@}" 2>/dev/null; then
		echo "[warn] image mounts not supported, falling back to polyfill"
		copy_from_image_polyfill "${@}"
	fi
}

if [ "${SKIP_KERNEL_BUILD}" != "1" ]; then
	echo "[kernel build] ${TARGET_ARCH} ${RUST_PROFILE}"
	docker build --platform="${DOCKER_TARGET}" -t "${DOCKER_PREFIX}/sprout-kernel-${TARGET_ARCH}:${DOCKER_TAG}" -f kernel/Dockerfile kernel

	if [ "${KERNEL_BUILD_TAG}" = "1" ]; then
		docker build --platform="${DOCKER_TARGET}" -t "${DOCKER_PREFIX}/sprout-kernel-build-${TARGET_ARCH}:${DOCKER_TAG}" -f kernel/Dockerfile --target
		build kernel
	fi

	copy_from_image "${DOCKER_PREFIX}/sprout-kernel-${TARGET_ARCH}" "kernel.efi" "${FINAL_DIR}/kernel.efi"
	cp "hack/configs/${SPROUT_CONFIG_NAME}.sprout.toml" "${FINAL_DIR}/sprout.toml"
	cp "hack/assets/edera-splash.png" "${FINAL_DIR}/edera-splash.png"
fi

if [ "${SKIP_VM_BUILD}" != "1" ]; then
	echo "[vm build] ${TARGET_ARCH} ${RUST_PROFILE}"
	docker build --platform="${DOCKER_TARGET}" -t "${DOCKER_PREFIX}/sprout-ovmf-${TARGET_ARCH}:${DOCKER_TAG}" -f vm/Dockerfile.ovmf "${FINAL_DIR}"
	copy_from_image "${DOCKER_PREFIX}/sprout-ovmf-${TARGET_ARCH}" "ovmf.fd" "${FINAL_DIR}/ovmf.fd"
	copy_from_image "${DOCKER_PREFIX}/sprout-ovmf-${TARGET_ARCH}" "shell.efi" "${FINAL_DIR}/shell.efi"
	docker build --platform="${DOCKER_TARGET}" -t "${DOCKER_PREFIX}/sprout-initramfs-${TARGET_ARCH}:${DOCKER_TAG}" -f vm/Dockerfile.initramfs "${FINAL_DIR}"
	copy_from_image "${DOCKER_PREFIX}/sprout-initramfs-${TARGET_ARCH}" "initramfs" "${FINAL_DIR}/initramfs"
fi

if [ "${SKIP_SPROUT_BUILD}" != "1" ]; then
	echo "[sprout build] ${TARGET_ARCH} ${RUST_PROFILE}"

	if [ "${SPROUT_BUILD_LOCAL}" = "1" ]; then
		cargo build --target "${RUST_TARGET}" --profile "${RUST_PROFILE}" --bin sprout
		cp "target/${RUST_TARGET}/${RUST_TARGET_SUBDIR}/sprout.efi" "${FINAL_DIR}/sprout.efi"
	else
		docker build --platform="${DOCKER_TARGET}" -t "${DOCKER_PREFIX}/sprout-${TARGET_ARCH}:${DOCKER_TAG}" --build-arg="RUST_TARGET_SUBDIR=${RUST_TARGET_SUBDIR}" -f Dockerfile .
		copy_from_image "${DOCKER_PREFIX}/sprout-${TARGET_ARCH}" "sprout.efi" "${FINAL_DIR}/sprout.efi"
	fi

	mkdir -p "${FINAL_DIR}/efi/EFI/BOOT"
	cp "${FINAL_DIR}/sprout.efi" "${FINAL_DIR}/efi/EFI/BOOT/${EFI_NAME}.EFI"
	if [ -f "${FINAL_DIR}/kernel.efi" ]; then
		cp "${FINAL_DIR}/kernel.efi" "${FINAL_DIR}/efi/EFI/BOOT/KERNEL.EFI"
	fi
	if [ -f "${FINAL_DIR}/shell.efi" ]; then
		cp "${FINAL_DIR}/shell.efi" "${FINAL_DIR}/efi/EFI/BOOT/SHELL.EFI"
	fi
	cp "${FINAL_DIR}/sprout.toml" "${FINAL_DIR}/efi/SPROUT.TOML"
	cp "${FINAL_DIR}/edera-splash.png" "${FINAL_DIR}/efi/EDERA-SPLASH.PNG"
	cp "${FINAL_DIR}/initramfs" "${FINAL_DIR}/efi/INITRAMFS"
fi

if [ "${SKIP_BOOT_BUILD}" != "1" ]; then
	echo "[boot build] ${TARGET_ARCH} ${RUST_PROFILE}"
	docker build --platform="${DOCKER_TARGET}" -t "${DOCKER_PREFIX}/sprout-boot-${TARGET_ARCH}:${DOCKER_TAG}" --build-arg "EFI_NAME=${EFI_NAME}" -f boot/Dockerfile "${FINAL_DIR}"
	copy_from_image "${DOCKER_PREFIX}/sprout-boot-${TARGET_ARCH}" "sprout.img" "${FINAL_DIR}/sprout.img"
fi
