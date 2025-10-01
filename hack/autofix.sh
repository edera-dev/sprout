#!/bin/sh
set -e

cd "$(dirname "${0}")/.." || exit 1

NATIVE_ARCH="$(uname -m)"
[ "${NATIVE_ARCH}" = "arm64" ] && NATIVE_ARCH="aarch64"
[ "${NATIVE_ARCH}" = "amd64" ] && NATIVE_ARCH="x86_64"

if [ "$(uname)" != "Linux" ]; then
	cargo clippy --workspace --fix --allow-dirty --allow-staged \
		--target "${NATIVE_ARCH}-unknown-uefi"
else
	cargo clippy --workspace --fix --allow-dirty --allow-staged
fi

./hack/format.sh
