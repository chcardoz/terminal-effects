#!/bin/bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
GENERATED="$(mktemp -d "${TMPDIR:-/tmp}/terminal-effects-editor-assets.XXXXXX")"
trap 'rm -rf "$GENERATED"' EXIT

(cd "$REPO_ROOT" && pnpm --filter @terminal-effects/editor exec vite build --outDir "$GENERATED")
diff -ru "$REPO_ROOT/apps/editor/dist" "$GENERATED"
