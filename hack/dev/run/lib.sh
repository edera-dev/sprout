#!/bin/sh
# Shared by the other scripts in this directory: source it after changing to the repository root.
# shellcheck disable=SC2034

# common.sh reads the architecture from the first argument, which belongs to the calling script here.
load_common() {
	set --
	. "hack/common.sh"
}
load_common

RUN_DIR="$(pwd)/target/run/${TARGET_ARCH}"
SHOT_DIR="${RUN_DIR}/shots"
SERIAL_LOG="${RUN_DIR}/serial.log"
CONSOLE_LOG="${RUN_DIR}/console.log"
QEMU_LOG="${RUN_DIR}/qemu.log"
PID_FILE="${RUN_DIR}/qemu.pid"

# Unix socket paths are limited to about 104 bytes on macOS, which a deep worktree exceeds.
QMP_SOCK="${TMPDIR:-/tmp}"
QMP_SOCK="${QMP_SOCK%/}/sprout-${TARGET_ARCH}.qmp"

die() {
	echo "ERROR: ${*}" >/dev/stderr
	exit 1
}

is_running() {
	[ -f "${PID_FILE}" ] && kill -0 "$(cat "${PID_FILE}")" 2>/dev/null
}

require_running() {
	is_running || die "no ${TARGET_ARCH} QEMU is running, start one with hack/dev/run/start.sh"
}
