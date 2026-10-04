# Sprout

Sprout is a programmable UEFI bootloader written in Rust, for `x86_64` and `aarch64`. It is a `no_std` workspace
built for the `*-unknown-uefi` targets, so it cannot be run or tested like a normal binary.
See [README.md](README.md) for the configuration format and features.

## Layout

- `crates/boot`: the bootloader entrypoint (`edera-sprout-boot`).
- `crates/bls`: Boot Loader Specification parsing and sorting.
- `crates/build`: build logic.
- `crates/config`: serialization structures for the Sprout configuration file.
- `crates/eficore`: core EFI helpers.
- `crates/parsing`: value stamping and other parsing helpers.
- `hack/`: build and dev scripts; `hack/dev` boots QEMU, `hack/dev/run` drives it headless.
- `docs/setup`: setup guides for signed and unsigned installs.

The crate list and the scripts are described in [DEVELOPMENT.md](DEVELOPMENT.md).

## Building and checking

The toolchain and both UEFI targets are pinned in `rust-toolchain.toml`. Always pass a UEFI target, as the
host target will not build.

```bash
cargo build --target x86_64-unknown-uefi
cargo clippy --target x86_64-unknown-uefi
cargo fmt --all
./hack/assemble.sh
```

CI runs `cargo fmt --check`, `cargo build` and `cargo clippy` for both targets, so run these for both
`x86_64` and `aarch64` before opening a PR. `./hack/autofix.sh` applies clippy and rustfmt fixes.

To try a change in a real environment, use `./hack/dev/boot.sh [arch]`, or the scripts in `hack/dev/run`
(for example `smoke.sh`) when there is no terminal or window to use.

## Conventions

- Write code that matches the surrounding code, including naming, idioms and comment style.
- Dependencies are declared in the root `Cargo.toml` under `[workspace.dependencies]`.
- GitHub Actions are pinned to a commit SHA with the version in a comment.

## Commits and pull requests

- Do not add coding agent attribution anywhere. No `Co-Authored-By` trailers, no "Generated with" lines, in
  commits or in PR descriptions.
- Commits use [Conventional Commits](https://www.conventionalcommits.org/), in the form `type(scope): subject`,
  for example `fix(boot): count tries without using them`. Types in use are `feat`, `fix` and `docs`, and
  scopes are usually a crate name such as `boot`, `bls` or `eficore`, or `ci`, `hack` and `readme`.
- PR titles are also conventional commits, because PRs are squashed. The only exception is a release PR,
  which is titled `sprout: version x.y.z`.
- Keep PR descriptions short prose.

## Releasing

See [RELEASING.md](RELEASING.md). A release PR only bumps the version in `Cargo.toml` and `Cargo.lock`.
