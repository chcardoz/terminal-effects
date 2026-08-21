# Changelog

All notable changes to Terminal Effects are documented here.

## Unreleased

### Changed

- Removed unused browser-app, recording, DevTools, registry, and database
  surfaces from the renderer while retaining every terminal adapter.
- Reorganized the mixed Rust and TypeScript codebase into `apps/`, `crates/`,
  `packages/`, and `tooling/` workspaces with one root Cargo and pnpm setup.
- Moved the direct installer implementation into `tooling/installer/` while
  preserving the root `install.sh` URL as a stable public bootstrap.

### Fixed

- Retried final filmstrip-frame extraction at a slightly earlier timestamp when
  FFmpeg cannot decode a frame exactly at the end of a media stream.

## 0.1.0 - 2026-08-20

### Added

- Initial terminal video editor, agent CLI, project model, FFmpeg media tools,
  and owned Chromium renderer for Kitty-graphics-compatible terminals.
