# Sprout Release Process

This document describes the steps required to publish a new Sprout release.

## Prerequisites

- Write access to the `edera-dev/sprout` repository
- GPG or SSH key configured for signed git tags
- `gh` CLI authenticated

## Steps

### 1. Bump the version

Update the version field in `Cargo.toml` (workspace root), then verify the build
succeeds:

```bash
./hack/build.sh
```

### 2. Open a version bump PR

Commit the `Cargo.toml` (and `Cargo.lock`) change with the message:

```txt
sprout: version x.y.z
```

After the PR is rebased and merged, the final commit message on `main` will read:

```txt
sprout: version x.y.z (#PR_NUMBER)
```

### 3. Create a signed tag

From an up-to-date `main`, create a signed tag for the release:

```bash
git tag -s vx.y.z
```

Use `vx.y.z` as the tag message (e.g. `v0.0.9`). Then push the tag:

```bash
git push origin vx.y.z
```

### 4. Draft the GitHub release

1. Navigate to **Releases → Draft a new release** on GitHub.
2. Select the tag created in the previous step.
3. Set the release title to the tag name (e.g. `v0.0.9`).
4. Click **Generate Release Notes** to populate the changelog.
5. Add a blank line after each section header if GitHub omitted them.
6. **Do not click Publish** — save as a draft.

### 5. Dispatch the release workflow

Trigger the [`release`](.github/workflows/release.yml) workflow manually:

1. Go to **Actions → release → Run workflow**.
2. Select the release tag as the branch/tag to run from.
3. Set the **Release Tag** input to the tag name (e.g. `v0.0.9`).
4. Click **Run workflow**.

The workflow will:

- Build release artifacts for `x86_64` and `aarch64` via `./hack/assemble.sh`
- Attach SLSA build provenance attestations to the artifacts
- Upload the artifacts to the draft release
- Mark the draft release as published

### 6. Done

Once the workflow completes successfully the release is live, with all artifacts
attached and attested before publication.
