#!/bin/bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"

"$REPO_ROOT/node_modules/.bin/esbuild" "$1" \
  --bundle --platform=node --format=cjs \
  --external:electron '--external:*.node' \
  --alias:@terminal-effects/pixel-react="$REPO_ROOT/packages/pixel-react/src/index.ts" \
  --alias:@terminal-effects/terminal-adapters="$REPO_ROOT/packages/terminal-adapters/src/index.ts" \
  --alias:@terminal-effects/renderer-runtime="$REPO_ROOT/packages/renderer-runtime/src/index.ts" \
  --define:process.env.NODE_ENV='"production"' \
  --sourcemap --outfile="$2" --log-level=warning

printf '{"type":"commonjs"}\n' > "$(dirname "$2")/package.json"
