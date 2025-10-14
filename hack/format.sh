#!/bin/sh
set -e

cd "$(dirname "${0}")/.." || exit 1

cargo fmt --all
shfmt -w hack/**/*.sh
