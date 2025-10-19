#!/bin/sh
set -e

cd "$(dirname "${0}")/.." || exit 1

. "hack/common.sh"

mkdir -p "${FINAL_DIR}"

cargo build --target "${RUST_TARGET}" --profile "${RUST_PROFILE}" --bin sprout
cp "target/${RUST_TARGET}/${RUST_TARGET_SUBDIR}/sprout.efi" "${FINAL_DIR}/sprout.efi"
