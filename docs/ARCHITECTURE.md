# Repository architecture

Terminal Effects is one product implemented by a Rust workspace and a pnpm
workspace. Source is organized by ownership and deployment role rather than by
language.

```text
apps/
  te/                         Rust CLI, editor server, and project model
  editor/                     React/Vite editor embedded in `te`
  renderer/
    browser/                  Electron offscreen rendering process
    launcher/                 Terminal-facing renderer launcher
    native/pixel-node/        N-API bridge into the native pixel engine
    assets/                   Renderer runtime assets
packages/
  pixel-react/                React reconciler for the native engine
  renderer-runtime/           Shared renderer paths and process state
  terminal-adapters/          Terminal detection and pane integrations
crates/
  pixel-core/                 Native terminal rendering engine
tooling/
  editor/                     Embedded-asset consistency checks
  installer/                  Direct-installer tests
  release/                    Complete platform archive builder
  renderer/                   Electron/runtime bundling scripts
```

The root `Cargo.toml` is authoritative for the product version and all Rust
members. The root `pnpm-workspace.yaml` and `pnpm-lock.yaml` are authoritative
for every JavaScript and TypeScript package. There are no nested workspaces.

## Runtime flow

`te` owns the project model and starts a session-scoped localhost server. The
server serves the editor assets embedded in the CLI. The packaged renderer opens
that URL in pinned Electron, paints it offscreen through `pixel-node` and
`pixel-core`, and sends frames through the selected terminal adapter.

The release archive layout is intentionally independent of the source layout:

```text
terminal-effects-VERSION-TARGET/
  bin/te
  libexec/terminal-effects-renderer/bin/te-renderer
```

Homebrew and the curl installer consume this stable archive layout, so source
reorganization does not affect installed paths.

## Root commands

- `pnpm check` type-checks every TypeScript project, tests every terminal
  adapter, and verifies the committed editor bundle.
- `cargo test --workspace` tests the CLI and the complete native renderer.
- `pnpm build:runtime` builds the matching local Electron renderer.
- `pnpm build` builds the editor and renderer runtime.
- `pnpm release:local [VERSION]` creates the complete release archive without
  publishing it.
