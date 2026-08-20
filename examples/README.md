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
te clips --json
te filmstrip 0s..12s --json
te screenshot --json
```
