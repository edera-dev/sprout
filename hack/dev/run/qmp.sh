#!/bin/sh
# Sends QMP commands to the running QEMU and prints each reply as a line of JSON.
# Usage: qmp.sh '{"execute":"query-status"}' ...
# Exits 1 if QEMU answers with an error. Environment: QMP_TIMEOUT is seconds per reply (default 30).
set -e

cd "$(dirname "${0}")/../../.." || exit 1

. "hack/dev/run/lib.sh"
require_running

[ "${#}" -gt 0 ] || die "usage: qmp.sh <json-command>..."

if command -v python3 >/dev/null 2>&1; then
	exec python3 - "${QMP_SOCK}" "${@}" <<'PY'
import json, os, socket, sys

path, *commands = sys.argv[1:]
sock = socket.socket(socket.AF_UNIX)
sock.settimeout(float(os.environ.get("QMP_TIMEOUT", "30")))
sock.connect(path)
stream = sock.makefile("rw")


def call(command):
    stream.write(command + "\n")
    stream.flush()
    while True:
        line = stream.readline()
        if not line:
            sys.exit("qmp: connection closed")
        message = json.loads(line)
        if "return" in message or "error" in message:
            return message


json.loads(stream.readline())
call('{"execute":"qmp_capabilities"}')
status = 0
for command in commands:
    reply = call(command)
    print(json.dumps(reply))
    if "error" in reply:
        status = 1
sys.exit(status)
PY
fi

command -v socat >/dev/null 2>&1 || die "python3 or socat is required"

# socat can't tell when a reply is complete, so it gets a fixed time to receive them.
OUTPUT="$(
	{
		printf '%s\n' '{"execute":"qmp_capabilities"}'
		for COMMAND in "${@}"; do
			printf '%s\n' "${COMMAND}"
		done
		sleep "${QMP_SETTLE:-1}"
	} | socat -t1 - "UNIX-CONNECT:${QMP_SOCK}"
)"
echo "${OUTPUT}"
case "${OUTPUT}" in
*'"error"'*) exit 1 ;;
esac
