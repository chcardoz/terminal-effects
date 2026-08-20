# Examples

`demo-project/` is an empty staging project for agent-editing experiments. Add
media files to that directory, then import them from the UI or with `te import`.
Video files and generated `.te/` caches are intentionally ignored by Git;
`project.teproj` remains tracked so each editing experiment can be reviewed.

From the demo directory:

```bash
cd examples/demo-project
cp /path/to/your/clips/*.mp4 .
te import ./*.mp4 --json
te .
```

Useful agent-side checks:

```bash
te status --json
te timeline
te assets --json
te clips --json
te add asset_f6f78c70dc --at 0s --source-in 0s --duration 12.3s --json
te duplicate clip_ab12 --at 30s --source-in 20s --duration 8s --json
te append asset_d51d2515c6 --source-in 80s --duration 10s --json
te filmstrip 0s..12s --json
te screenshot --json
```

To inspect the Chromium UI in a normal browser instead of through the terminal
frame transport:

```bash
te serve . --port 4173
```
