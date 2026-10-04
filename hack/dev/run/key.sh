#!/bin/sh
# Presses keys in the guest, one after another. Join keys with + to press them together.
# Usage: key.sh <qcode>...   for example: key.sh down down ret   or: key.sh ctrl+alt+delete
# Environment: KEY_DELAY is seconds between keys (default 0.3).
set -e

cd "$(dirname "${0}")/../../.." || exit 1

. "hack/dev/run/lib.sh"
require_running

[ "${#}" -gt 0 ] || die "usage: key.sh <qcode>..."

FIRST=1
for CHORD in "${@}"; do
	[ "${FIRST}" = "1" ] || sleep "${KEY_DELAY:-0.3}"
	FIRST=0
	KEYS=""
	OLD_IFS="${IFS}"
	IFS="+"
	for KEY in ${CHORD}; do
		KEYS="${KEYS}${KEYS:+,}{\"type\":\"qcode\",\"data\":\"${KEY}\"}"
	done
	IFS="${OLD_IFS}"
	./hack/dev/run/qmp.sh "{\"execute\":\"send-key\",\"arguments\":{\"keys\":[${KEYS}]}}" >/dev/null
done
