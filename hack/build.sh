#!/bin/sh
set -e

cd "$(dirname "${0}")/.." || exit 1

. "hack/common.sh"

EFI_NAME="BOOTX64"
if [ "${TARGET_ARCH}" = "aarch64" ]; then
	EFI_NAME="BOOTAA64"
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
	docker build -t "${DOCKER_PREFIX}/sprout-utils-copy:${DOCKER_TAG}" -f hack/utils/Dockerfile.copy hack
fi

if [ "${SKIP_KERNEL_BUILD}" != "1" ]; then
	echo "[kernel build] ${TARGET_ARCH} ${RUST_PROFILE}"
	docker build --platform="${DOCKER_TARGET}" -t "${DOCKER_PREFIX}/sprout-kernel-${TARGET_ARCH}:${DOCKER_TAG}" -f kernel/Dockerfile kernel

	if [ "${KERNEL_BUILD_TAG}" = "1" ]; then
		docker build --platform="${DOCKER_TARGET}" -t "${DOCKER_PREFIX}/sprout-kernel-build-${TARGET_ARCH}:${DOCKER_TAG}" -f kernel/Dockerfile --target
		build kernel
	fi

	docker run --rm -i \
		--mount="type=image,source=${DOCKER_PREFIX}/sprout-kernel-${TARGET_ARCH}:${DOCKER_TAG},target=/image" \
		"${DOCKER_PREFIX}/sprout-utils-copy:${DOCKER_TAG}" cat /image/kernel.efi >"${FINAL_DIR}/kernel.efi"
fi

if [ "${SKIP_VM_BUILD}" != "1" ]; then
	echo "[vm build] ${TARGET_ARCH} ${RUST_PROFILE}"
	docker build --platform="${DOCKER_TARGET}" -t "${DOCKER_PREFIX}/sprout-ovmf-${TARGET_ARCH}:${DOCKER_TAG}" -f vm/Dockerfile.ovmf "${FINAL_DIR}"
	docker run --rm -i \
		--mount="type=image,source=${DOCKER_PREFIX}/sprout-ovmf-${TARGET_ARCH}:${DOCKER_TAG},target=/image" \
		"${DOCKER_PREFIX}/sprout-utils-copy:${DOCKER_TAG}" cat /image/ovmf.fd >"${FINAL_DIR}/ovmf.fd"
fi

if [ "${SKIP_SPROUT_BUILD}" != "1" ]; then
	echo "[sprout build] ${TARGET_ARCH} ${RUST_PROFILE}"
	docker build --platform="${DOCKER_TARGET}" -t "${DOCKER_PREFIX}/sprout-${TARGET_ARCH}:${DOCKER_TAG}" --build-arg="RUST_TARGET_SUBDIR=${RUST_TARGET_SUBDIR}" -f Dockerfile .
	docker run --rm -i \
		--mount="type=image,source=${DOCKER_PREFIX}/sprout-${TARGET_ARCH}:${DOCKER_TAG},target=/image" \
		"${DOCKER_PREFIX}/sprout-utils-copy:${DOCKER_TAG}" cat /image/sprout.efi >"${FINAL_DIR}/sprout.efi"
	mkdir -p "${FINAL_DIR}/efi/EFI/BOOT"
	cp "${FINAL_DIR}/sprout.efi" "${FINAL_DIR}/efi/EFI/BOOT/${EFI_NAME}.EFI"
	if [ -f "${FINAL_DIR}/kernel.efi" ]; then
		cp "${FINAL_DIR}/kernel.efi" "${FINAL_DIR}/efi/EFI/BOOT/KERNEL.EFI"
	fi
fi

if [ "${SKIP_BOOT_BUILD}" != "1" ]; then
	echo "[boot build] ${TARGET_ARCH} ${RUST_PROFILE}"
	docker build --platform="${DOCKER_TARGET}" -t "${DOCKER_PREFIX}/sprout-boot-${TARGET_ARCH}:${DOCKER_TAG}" --build-arg "EFI_NAME=${EFI_NAME}" -f boot/Dockerfile "${FINAL_DIR}"
	docker run --rm -i \
		--mount="type=image,source=${DOCKER_PREFIX}/sprout-boot-${TARGET_ARCH}:${DOCKER_TAG},target=/image" \
		"${DOCKER_PREFIX}/sprout-utils-copy:${DOCKER_TAG}" cat /image/sprout.img >"${FINAL_DIR}/sprout.img"
fi
