#!/bin/bash
set -e
# Without globstar, ** matches like *, so only scripts one directory deep are formatted.
shopt -s globstar

cd "$(dirname "${0}")/.." || exit 1

cargo fmt --all || true
shfmt -w hack/**/*.sh || true
