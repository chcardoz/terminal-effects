# Releasing Terminal Effects

Releases are intentionally manual. Pull requests and pushes to `main` run CI
but never publish a version.

## Prepare

1. Decide whether the release is stable (`0.2.0`) or a prerelease
   (`0.2.0-beta.1`).
2. Update the workspace version in `Cargo.toml` and run `cargo check --workspace` to refresh
   `Cargo.lock` if necessary.
3. Move the relevant entries in `CHANGELOG.md` out of `Unreleased`.
4. Merge those changes into `main`.

## Build the draft

Open **Actions → Release → Run workflow** in GitHub. Enter the exact
`Cargo.toml` version, choose `stable` or `prerelease`, and run it from `main`.

The workflow builds and smoke-tests these native packages:

- `darwin-arm64`
- `linux-x64`
- `linux-arm64`

It creates a draft GitHub Release only after every build succeeds. Review the
generated notes and assets, then explicitly publish the draft from GitHub.

The same workflow can be started from a terminal:

```bash
gh workflow run release.yml --ref main -f version=0.2.0 -f channel=stable
```

To build the same archive locally without publishing anything, run:

```bash
pnpm release:local 0.2.0
```

## Homebrew

The public [`chcardoz/homebrew-tap`](https://github.com/chcardoz/homebrew-tap)
repository checks once per day for a newly published stable release and updates
its cask. Drafts and prereleases are ignored.

To update it immediately after publishing a stable release:

```bash
gh workflow run update-formula.yml --repo chcardoz/homebrew-tap -f version=0.2.0
```

No cross-repository personal access token is needed. The tap workflow writes
only to the tap using its repository-scoped GitHub token.

## Direct installer

The root `install.sh` is a stable public bootstrap for the documented raw
GitHub URL. It downloads the implementation in
`tooling/installer/install.sh`, which then downloads the assets behind GitHub's
latest stable-release redirect. Publishing a stable release therefore updates
the curl installation path without changing either script. Prereleases do not
affect it.
