#!/bin/bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
VERSION="${1:-$(sed -n 's/^version = "\([^"]*\)"/\1/p' "$ROOT/Cargo.toml" | head -n 1)}"

case "$(uname -s)-$(uname -m)" in
  Darwin-arm64) TARGET=darwin-arm64 ;;
  Linux-x86_64|Linux-amd64) TARGET=linux-x64 ;;
  Linux-aarch64|Linux-arm64) TARGET=linux-arm64 ;;
  *) echo "unsupported release host: $(uname -s)-$(uname -m)" >&2; exit 1 ;;
esac

(cd "$ROOT/web" && npm ci && npm run build)
(cd "$ROOT/renderer" && pnpm install --frozen-lockfile && pnpm build:runtime "$VERSION")
(cd "$ROOT" && cargo build --release)

OUT="$ROOT/dist"
PACKAGE="terminal-effects-$VERSION-$TARGET"
STAGE="$OUT/$PACKAGE"
rm -rf "$OUT"
mkdir -p "$STAGE/bin" "$STAGE/libexec"
cp "$ROOT/target/release/te" "$STAGE/bin/te"
cp -R "$ROOT/renderer/dist/terminal-effects-renderer" \
  "$STAGE/libexec/terminal-effects-renderer"
cp "$ROOT/README.md" "$ROOT/THIRD_PARTY_NOTICES.md" "$STAGE/"
cp "$ROOT/renderer/LICENSE.terminal-browser" "$STAGE/"

TARBALL="$OUT/$PACKAGE.tar.gz"
tar -czf "$TARBALL" -C "$OUT" "$PACKAGE"
if command -v shasum >/dev/null 2>&1; then
  shasum -a 256 "$TARBALL" > "$TARBALL.sha256"
else
  sha256sum "$TARBALL" > "$TARBALL.sha256"
fi

du -h "$TARBALL"
