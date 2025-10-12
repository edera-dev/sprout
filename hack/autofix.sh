#!/bin/sh
set -e

cd "$(dirname "${0}")/.." || exit 1

. "hack/common.sh"

cargo clippy --workspace --fix --allow-dirty --allow-staged --target "${HOST_ARCH}-unknown-uefi"
./hack/format.sh
