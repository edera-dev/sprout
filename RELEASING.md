# Releasing Sprout

This describes how a maintainer cuts a release of Sprout. A release is a signed tag plus a GitHub release
that holds the built EFI binaries and their attestations.

The release is built and published by the `release` workflow, so the release is never visible
until its artifacts are attached.

## Steps

1. Bump `version` in the `[workspace.package]` section of `Cargo.toml` to the new version, then run
   `./hack/assemble.sh` to update `Cargo.lock`.

2. Open a pull request with the result. The commit and the PR title are `sprout: version x.y.z`, which is the
   one place we do not use a conventional commit. Once squashed, the commit on `main` is
   `sprout: version x.y.z (#PR_NUMBER)`.

3. Once the PR has merged, create a signed tag on the merged commit. The tag message is just the tag name.

   ```bash
   git tag -s v0.0.8 -m v0.0.8
   git push origin v0.0.8
   ```

4. Draft a new release on GitHub for that tag, named after the tag, and click Generate Release Notes.
   Add a blank line after every section header, as GitHub does not always do that. **Do not click Publish.**

5. Run the `release` workflow from the Actions tab, selecting the tag as the ref to run from, and set the
   `Release Tag` input to the tag name (for example `v0.0.8`). The workflow builds the tagged commit,
   uploads the binaries to the draft release, attests them, and then publishes the release.

## Notes

- The release workflow builds the tag, not the branch it was dispatched on, so check the tag points at the
  version bump commit before running it.
- Re-running the workflow for the same tag replaces the uploaded assets, and runs for one tag never overlap.
