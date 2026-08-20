# Terminal Effects

A video editor written directly in Rust and rendered in compatible terminals with the Kitty graphics protocol. Human interactions and agent commands share the same validated project model and editing functions.

## Start a project

```bash
mkdir my-edit && cd my-edit
cp /path/to/video.mp4 .
te .
```

`te .` creates `project.teproj`, imports media in the directory, and opens the graphical terminal editor. Project discovery walks upward from the current directory, like Git.

## Agent surface

```bash
te status --json
te timeline
te clips --json
te frame 12.5s --json
te filmstrip 10s..20s --json
te screenshot --json
te split clip_ab12 14.2s --json
te move clip_ab12 --track V1 --at 20s --json
te remove clip_ab12 --json
te undo --json
te export result.mp4 --json
```

IDs accept an unambiguous prefix. Times accept frames (`45f`), seconds (`1.5s`), bare seconds, or timecodes (`00:01.500`, `00:00:01.500`).

## Human controls

- Arrow keys scrub one frame; Shift+Arrow scrubs one second.
- Tab selects the next clip.
- `s` splits the selected clip at the playhead.
- Delete removes the selected clip.
- Space toggles a low-frame-rate preview.
- Ctrl+Z / Ctrl+Y undo and redo.
- `e` exports to `.te/exports/export.mp4`.
- `q` or Escape quits.

FFmpeg and FFprobe must be available on `PATH`.

## Local demo

This checkout includes a gitignored, real-media demo project prepared from videos already on this machine:

```bash
cd examples/demo-project
te .
```

See [`examples/README.md`](examples/README.md) for agent-side commands.
