# Examples

`demo-project/` is a local test project populated from media already on this machine. Video files and generated `.te/` caches are intentionally ignored by Git; `project.teproj` remains readable so the project model can be inspected.

From the demo directory:

```bash
cd examples/demo-project
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
