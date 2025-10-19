#!/bin/sh
set -e

retry() {
	for i in $(seq 1 10); do
		if "${@}"; then
			return 0
		else
			sleep "${i}"
		fi
	done
	"${@}"
}

if [ -z "${RELEASE_TAG}" ]; then
	exit 1
fi

cd target/assemble

retry gh release upload "${TAG}" --clobber ./*
