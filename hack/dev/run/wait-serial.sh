#!/bin/sh
# Waits for a regex to show up on the firmware or Linux console. Exits 1 on timeout or if QEMU exits.
# Usage: wait-serial.sh <regex> [timeout-seconds]
set -e

cd "$(dirname "${0}")/../../.." || exit 1

. "hack/dev/run/lib.sh"

[ -n "${1}" ] || die "usage: wait-serial.sh <regex> [timeout-seconds]"

DEADLINE=$(($(date +%s) + ${2:-60}))
while :; do
	if cat "${SERIAL_LOG}" "${CONSOLE_LOG}" 2>/dev/null | grep -aEq -- "${1}"; then
		exit 0
	fi
	if ! is_running; then
		echo "QEMU exited while waiting for: ${1}" >/dev/stderr
		break
	fi
	if [ "$(date +%s)" -ge "${DEADLINE}" ]; then
		echo "timed out waiting for: ${1}" >/dev/stderr
		break
	fi
	sleep 0.5
done

tail -n 15 "${SERIAL_LOG}" "${CONSOLE_LOG}" 2>/dev/null >/dev/stderr || true
exit 1
