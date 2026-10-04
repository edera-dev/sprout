#!/bin/sh
# Starts the dev environment in the background with no window, a QMP socket and serial logs.
# Usage: start.sh [arch]
# Environment: SKIP_BUILD=1 reuses target/final, SPROUT_CONFIG_NAME picks the config (default
# graphical), START_TIMEOUT is the seconds to wait for QEMU (default 900 to allow for a build).
set -e

cd "$(dirname "${0}")/../../.." || exit 1

[ -n "${1}" ] && TARGET_ARCH="${1}"
. "hack/dev/run/lib.sh"

is_running && die "a ${TARGET_ARCH} QEMU is already running, stop it with hack/dev/run/stop.sh"

rm -rf "${RUN_DIR}"
mkdir -p "${SHOT_DIR}"
rm -f "${QMP_SOCK}"

export SPROUT_CONFIG_NAME="${SPROUT_CONFIG_NAME:-graphical}"
export QEMU_HEADLESS=1
export QEMU_QMP="${QMP_SOCK}"
export QEMU_SERIAL_FILE="${SERIAL_LOG}"
export QEMU_CONSOLE_FILE="${CONSOLE_LOG}"

nohup ./hack/dev/boot.sh "${TARGET_ARCH}" </dev/null >"${QEMU_LOG}" 2>&1 &
echo "${!}" >"${PID_FILE}"

# boot.sh builds first and then execs QEMU, so the pid stays the same.
DEADLINE=$(($(date +%s) + ${START_TIMEOUT:-900}))
while [ ! -S "${QMP_SOCK}" ]; do
	if ! is_running; then
		tail -n 20 "${QEMU_LOG}" >/dev/stderr
		die "QEMU exited before it was ready, see ${QEMU_LOG}"
	fi
	[ "$(date +%s)" -ge "${DEADLINE}" ] && die "timed out waiting for QEMU, see ${QEMU_LOG}"
	sleep 1
done

echo "${RUN_DIR}"
