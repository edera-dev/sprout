#!/bin/sh
# Saves a screenshot of the guest display as a PNG and prints its path, ready to be read as an image.
# Usage: screenshot.sh [out.png]
# Requires QEMU 7.1 or newer for PNG screendumps.
set -e

cd "$(dirname "${0}")/../../.." || exit 1

. "hack/dev/run/lib.sh"
require_running

OUT="${1:-${SHOT_DIR}/$(date +%H%M%S).png}"
case "${OUT}" in
/*) ;;
*) OUT="$(pwd)/${OUT}" ;;
esac
mkdir -p "$(dirname "${OUT}")"

./hack/dev/run/qmp.sh "{\"execute\":\"screendump\",\"arguments\":{\"filename\":\"${OUT}\",\"format\":\"png\"}}" >/dev/null
[ -s "${OUT}" ] || die "QEMU did not write ${OUT}"
echo "${OUT}"
