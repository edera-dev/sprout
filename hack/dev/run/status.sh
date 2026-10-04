#!/bin/sh
# Prints the state of QEMU: stopped, or the pid and the VM status such as running.
# Exits 1 when QEMU isn't running.
set -e

cd "$(dirname "${0}")/../../.." || exit 1

. "hack/dev/run/lib.sh"

if ! is_running; then
	echo "stopped"
	exit 1
fi

echo "pid $(cat "${PID_FILE}") $(./hack/dev/run/qmp.sh '{"execute":"query-status"}' 2>/dev/null || echo "no qmp")"
