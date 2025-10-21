#!/bin/sh
# shellcheck disable=SC2034
set -e

DOCKER_PREFIX="ghcr.io/edera-dev/sprout"
DEFAULT_RUST_PROFILE="release"
DEFAULT_DOCKER_TAG="latest"

HOST_ARCH="$(uname -m)"

[ "${HOST_ARCH}" = "arm64" ] && HOST_ARCH="aarch64"
[ "${HOST_ARCH}" = "amd64" ] && HOST_ARCH="x86_64"

[ -z "${TARGET_ARCH}" ] && TARGET_ARCH="${1}"
{ [ -z "${TARGET_ARCH}" ] || [ "${TARGET_ARCH}" = "native" ]; } && TARGET_ARCH="${HOST_ARCH}"
[ -z "${RUST_PROFILE}" ] && RUST_PROFILE="${2}"
[ -z "${RUST_PROFILE}" ] && RUST_PROFILE="${DEFAULT_RUST_PROFILE}"

[ "${TARGET_ARCH}" = "arm64" ] && TARGET_ARCH="aarch64"
[ "${TARGET_ARCH}" = "amd64" ] && TARGET_ARCH="x86_64"

if [ "${TARGET_ARCH}" != "x86_64" ] && [ "${TARGET_ARCH}" != "aarch64" ]; then
	echo "Unsupported architecture: ${TARGET_ARCH}" >/dev/stderr
	exit 1
fi

[ "${RUST_PROFILE}" = "debug" ] && RUST_PROFILE="dev"

RUST_TARGET_SUBDIR="${RUST_PROFILE}"
[ "${RUST_PROFILE}" = "dev" ] && RUST_TARGET_SUBDIR="debug"

RUST_TARGET="${TARGET_ARCH}-unknown-uefi"

[ -z "${DOCKER_TAG}" ] && DOCKER_TAG="${DEFAULT_DOCKER_TAG}"
DOCKER_TARGET="linux/${TARGET_ARCH}"
FINAL_DIR="target/final/${TARGET_ARCH}"
ASSEMBLE_DIR="target/assemble"

if [ -z "${QEMU_ACCEL}" ] && [ "${TARGET_ARCH}" = "${HOST_ARCH}" ] &&
	[ -f "/proc/cpuinfo" ] &&
	grep -E '^flags.*:.+ vmx .*' /proc/cpuinfo >/dev/null; then
	QEMU_ACCEL="kvm"
fi

if [ "$(uname)" = "Darwin" ] && [ "${TARGET_ARCH}" = "${HOST_ARCH}" ] &&
	[ "$(sysctl -n kern.hv_support 2>&1 || true)" = "1" ]; then
	QEMU_ACCEL="hvf"
fi
