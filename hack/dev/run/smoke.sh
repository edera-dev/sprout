#!/bin/sh
# Boots the graphical menu, moves the selection up and down, boots Linux and waits for its shell prompt.
# Usage: smoke.sh [arch]
# Screenshots land in target/run/<arch>/shots. The exit code is the result.
# Environment: SMOKE_MENU_DELAY is seconds to let the menu draw (default 10), SMOKE_BOOT_TIMEOUT is
# seconds to wait for the shell (default 180). The start.sh variables apply too.
set -e

cd "$(dirname "${0}")/../../.." || exit 1

[ -n "${1}" ] && TARGET_ARCH="${1}"
. "hack/dev/run/lib.sh"
export TARGET_ARCH

trap './hack/dev/run/stop.sh' EXIT

./hack/dev/run/start.sh "${TARGET_ARCH}"
sleep "${SMOKE_MENU_DELAY:-10}"

echo "menu: $(./hack/dev/run/screenshot.sh "${SHOT_DIR}/menu.png")"
# The default entry is the last one, so up is the key that visibly moves the selection.
./hack/dev/run/key.sh up
echo "up: $(./hack/dev/run/screenshot.sh "${SHOT_DIR}/menu-up.png")"
./hack/dev/run/key.sh down ret

./hack/dev/run/wait-serial.sh 'sprout:.*# ' "${SMOKE_BOOT_TIMEOUT:-180}"
echo "ok: reached a shell on ${TARGET_ARCH}"
