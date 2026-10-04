---
name: update-config-docs
description: Bring docs/config.md back in line with the code. Use after changing anything in crates/config, the command line options, loader.conf handling, strict mode, or autoconfiguration, or when asked to check that docs/config.md is accurate.
---

# Update docs/config.md

`docs/config.md` is the reference for `sprout.toml`, the command line options, BLS support and strict mode.
The README only links to it, so a setting that is missing from docs/config.md is undocumented.

## 1. Find what changed

Start from the diff, or from the whole config surface if there is none:

```bash
git diff main -- crates/config crates/boot/src/options.rs crates/boot/src/config crates/boot/src/autoconfigure crates/bls
```

The sources of truth, and the section of docs/config.md each one feeds:

| Source                                                    | Section                                   |
|-----------------------------------------------------------|-------------------------------------------|
| `crates/config/src/lib.rs`                                | file at a glance, `options`               |
| `crates/config/src/drivers.rs`                            | `drivers`                                 |
| `crates/config/src/extractors*`                           | `extractors`                              |
| `crates/config/src/actions*`                              | `actions`                                 |
| `crates/config/src/entries.rs`                            | `entries`                                 |
| `crates/config/src/generators*`                           | `generators`, `variants`, `exclude`       |
| `crates/config/src/phases.rs`                             | `phases`                                  |
| `crates/boot/src/options.rs`                              | command line options                      |
| `crates/boot/src/main.rs`                                 | boot order, defaults, timeout and default entry order, strict mode |
| `crates/boot/src/autoconfigure*`                          | autoconfiguration                         |
| `crates/boot/src/generators/bls.rs`, `crates/bls`         | BLS values, boot counting, `loader.conf`  |

## 2. Check each key

For every struct field in `crates/config`, docs/config.md needs its TOML name (look at `#[serde(rename)]`, as
most multi-word keys use dashes), its type, its default, and what it does. Read the code that uses it in
`crates/boot`, not only the doc comment, because the behavior is what the reader depends on.

Look for the usual drift:

- a field added, renamed or removed
- a default that changed (`#[serde(default = ...)]`, the `Default` impls, constants such as
  `DEFAULT_MENU_TIMEOUT_SECONDS`)
- a new BLS value set with `context.set`, or a new option in `SproutOptions`
- a new row to add to, or remove from, the strict mode table
- a changed order: the default entry sources, the menu timeout sources, or the phases
- `LATEST_VERSION` changing, which the intro and `version` text must follow

Don't document what you can't confirm in the code. If a behavior is unclear, read the function that
implements it.

## 3. Write the change

- Keep each section's shape: a short explanation, a TOML example, then a table of keys.
- Examples must parse. Check that every key in one exists and is spelled the way serde expects.
- Update the table of contents if you add or rename a heading.
- Keep README.md short. It gets the feature list and the minimal examples, and docs/config.md gets everything else.
- Don't duplicate the strict mode table or the default entry order anywhere else.

## 4. Write like a person

Plain sentences, said once. Skip these:

- filler like "robust", "seamless", "powerful", "leverage", "ensure", "note that", "it's important to"
- groups of three added for rhythm, and a summary sentence at the end of every section
- bullet lists for what a sentence says better, and bold used as decoration
- em dashes (use a comma, colon or parentheses)

Say what a setting does and what happens when it's unset or wrong. Name real things (`bootctl`, `PCR 12`,
`\EFI\Linux`) instead of describing them.

## 5. Verify

```bash
cargo build --target x86_64-unknown-uefi
cargo build --target aarch64-unknown-uefi
cargo fmt --all --check
```

Then re-read the diff of docs/config.md against the code once more for any key whose name or default you
touched. If the change was only to docs, skip the builds.
