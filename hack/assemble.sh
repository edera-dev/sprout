#!/bin/sh
set -e

cd "$(dirname "${0}")/.." || exit 1

. "hack/common.sh"

mkdir -p "${ASSEMBLE_DIR}"

build_arch() {
  ARCHITECTURE="${1}"
  TARGET_ARCH="${ARCHITECTURE}" ./hack/build.sh
  cp "target/final/${ARCHITECTURE}/sprout.efi" "${ASSEMBLE_DIR}/sprout-${ARCHITECTURE}.efi"
}

build_arch x86_64
build_arch aarch64
