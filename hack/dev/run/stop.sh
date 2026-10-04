#!/bin/sh
# Stops QEMU and removes its socket. Safe to run when nothing is running.
set -e

cd "$(dirname "${0}")/../../.." || exit 1

. "hack/dev/run/lib.sh"

if is_running; then
	PID="$(cat "${PID_FILE}")"
	./hack/dev/run/qmp.sh '{"execute":"quit"}' >/dev/null 2>&1 || true
	for _ in 1 2 3 4 5 6 7 8 9 10; do
		is_running || break
		sleep 0.5
	done
	is_running && kill "${PID}" 2>/dev/null || true
fi

rm -f "${QMP_SOCK}" "${PID_FILE}"
