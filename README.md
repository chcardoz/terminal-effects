# Terminal Effects

A real video editor that runs inside the terminal. A Terminal Effects-owned
renderer uses a pinned Chromium build to render the complete human interface
offscreen and sends those frames to Ghostty, Kitty, and other Kitty-graphics
terminals. Rust remains the project, editing, media, and agent-command backend.

```text
Vite + React + TypeScript + <video>
        ↓ Chromium offscreen frames
Terminal Effects renderer → Kitty graphics → terminal
        ↕ local authenticated HTTP
Rust project model + FFmpeg + agent CLI
```

## Install

Terminal Effects currently supports Apple Silicon macOS, x86-64 Linux, and
ARM64 Linux. Install the latest stable release with Homebrew:

```bash
brew install --cask chcardoz/tap/te
```

Or install directly from GitHub without Homebrew:

```bash
curl -fsSL https://raw.githubusercontent.com/chcardoz/terminal-effects/main/install.sh | sh
```

The direct installer verifies the release checksum and installs into
`~/.local/share/terminal-effects`, with the `te` link in `~/.local/bin`.
FFmpeg and FFprobe must be available on `PATH`. Use Ghostty on macOS, Kitty on
Linux, or another terminal that supports the Kitty graphics protocol.

## Start a project

```bash
mkdir my-edit && cd my-edit
cp /path/to/video.mp4 .
te .
```

`te .` creates `project.teproj`, imports media in the directory, and opens the graphical terminal editor. Project discovery walks upward from the current directory, like Git.

Release packages contain `te` and its matching Chromium renderer. Nothing is
downloaded on first launch, and Terminal Effects does not use Chrome or another
browser installed on the computer. `te runtime --json` shows the packaged
renderer selected by the executable. `TE_RENDERER_BIN` can select a development
renderer explicitly.

For ordinary browser development or UI debugging:

```bash
te serve . --port 4173
```

The command prints a session-scoped localhost URL. The terminal version and the
ordinary-browser version use the exact same UI and Rust API.

The UI source lives in `web/src/`. The production Vite bundle is committed
because Rust embeds it in the `te` executable. After editing the interface:

```bash
cd web
npm install
npm run build
cd ..
cd renderer
pnpm install
pnpm build:runtime
cd ..
cargo test
```

Renderer sources live in `renderer/`. The packaged runtime contains only the
Terminal Effects launcher, offscreen browser process, native pixel bridge,
terminal integration, and pinned Electron/Chromium build. General browser CLI,
agent-browser, split-pane commands, skills, and browser installation machinery
are not shipped. Maintainers can produce a complete platform archive with
`scripts/build-release.sh`.

Maintainers publish macOS and Linux packages through a manually triggered,
draft-first GitHub workflow. See [`docs/RELEASING.md`](docs/RELEASING.md).

For Vite hot reload, run `te serve . --port 4173`, copy its session URL, and
start `npm run dev` from `web/` with `TE_API_TARGET` set to that URL's origin and
session prefix.

## Agent surface

```bash
te status --json
te timeline
te assets --json
te clips --json
te add asset_ab12 --track V1 --at 0s --source-in 3s --duration 12s --json
te duplicate clip_ab12 --at 20s --source-in 30s --duration 8s --json
te append asset_ab12 --source-in 45s --duration 10s --json
te transform clip_ab12 --rotate 90 --fit cover --position-x 0.5 --position-y 0.35 --json
te frame 12.5s --json
te filmstrip 10s..20s --json
te screenshot --json
te split clip_ab12 14.2s --json
te move clip_ab12 --track V1 --at 20s --json
te trim clip_ab12 --start 20s --source-in 2s --duration 8s --json
te remove clip_ab12 --json
te undo --json
te export result.mp4 --json
```

`te screenshot` returns the current FFmpeg-rendered program frame. IDs accept an
unambiguous prefix. Times accept frames (`45f`), seconds (`1.5s`), bare seconds,
or timecodes (`00:01.500`, `00:00:01.500`).

`te add` places an asset at an exact timeline position. `te duplicate` creates a
new clip from an existing clip's asset and inherits its source range unless an
override is supplied. `te append` places an asset at the end of its compatible
track. Duration defaults to the remaining source media for `add` and `append`.
All three placement commands are undoable and report newly created clip IDs in
the JSON `created` array.

`te transform` changes only a clip's presentation. Rotation is absolute and
accepts `0`, `90`, `180`, or `270` degrees. `contain` preserves the entire frame;
`cover` fills the project frame and uses normalized focal positions from `0` to
`1`. Use `te transform CLIP --reset` to restore centered, unrotated `contain`.
Transforms are non-destructive, undoable, and applied consistently to browser
playback, FFmpeg previews, filmstrips, and exports.

## Human editor

- Arrow keys scrub one frame; Shift+Arrow scrubs one second.
- Space plays and pauses real browser video.
- `s` splits the selected clip at the playhead; Delete removes it.
- `b` selects the blade tool and `v` returns to the selection tool.
- Command/Ctrl+Z and Shift+Command/Ctrl+Z undo and redo.
- Drag clips to move them and drag either edge to trim.
- The inspector exposes exact frame values for deterministic timing edits.
- Import accepts one absolute media path per line.
- Export writes `.te/exports/export.mp4` with FFmpeg.
- `q` closes the terminal editor.

FFmpeg and FFprobe must be available on `PATH`.

## Agent behavior

The browser is presentation, not source of truth. Every interaction is validated
by the same Rust functions used by the JSON CLI. The editor polls the project for
external revision changes, so an agent can add, duplicate, append, split, move,
trim, import, undo, or export while a human has the editor open and the UI
updates automatically.

## Local demo

This checkout includes an empty staging project for local media experiments:

```bash
cd examples/demo-project
cp /path/to/your/clips/*.mp4 .
te import ./*.mp4 --json
te .
```

See [`examples/README.md`](examples/README.md) for agent-side commands.
