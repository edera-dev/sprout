#!/bin/sh
set -e

cd "$(dirname "${0}")/.." || exit 1

. "hack/common.sh"

delete_image() {
	IMAGE="${1}"
	docker image ls -q --no-trunc --filter "reference=${DOCKER_PREFIX}/${IMAGE}" | xargs -rn1 docker image rm
}

cargo clean || true

if command -v docker >/dev/null 2>&1; then
	delete_image sprout-x86_64 || true
	delete_image sprout-aarch64 || true
	delete_image sprout-utils-copy-direct || true
	delete_image sprout-utils-copy-polyfill || true
	delete_image sprout-ovmf-x86_64 || true
	delete_image sprout-ovmf-aarch64 || true
	delete_image sprout-initramfs-x86_64 || true
	delete_image sprout-initramfs-aarch64 || true
	delete_image sprout-kernel-x86_64 || true
	delete_image sprout-kernel-aarch64 || true
	delete_image sprout-kernel-build-x86_64 || true
	delete_image sprout-kernel-build-aarch64 || true
	delete_image sprout-boot-x86_64 || true
	delete_image sprout-boot-aarch64 || true
	delete_image sprout-xen-x86_64 || true
	delete_image sprout-xen-aarch64 || true
fi
