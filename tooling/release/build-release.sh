#!/bin/bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
VERSION="${1:-$(sed -n 's/^version = "\([^"]*\)"/\1/p' "$REPO_ROOT/Cargo.toml" | head -n 1)}"

case "$(uname -s)-$(uname -m)" in
  Darwin-arm64) TARGET=darwin-arm64 ;;
  Linux-x86_64|Linux-amd64) TARGET=linux-x64 ;;
  Linux-aarch64|Linux-arm64) TARGET=linux-arm64 ;;
  *) echo "unsupported release host: $(uname -s)-$(uname -m)" >&2; exit 1 ;;
esac

(cd "$REPO_ROOT" && pnpm install --frozen-lockfile)
(cd "$REPO_ROOT" && pnpm build:editor)
(cd "$REPO_ROOT" && pnpm build:runtime "$VERSION")
(cd "$REPO_ROOT" && cargo build --release -p terminal-effects)

OUT="$REPO_ROOT/dist"
PACKAGE="terminal-effects-$VERSION-$TARGET"
STAGE="$OUT/$PACKAGE"
rm -rf "$OUT"
mkdir -p "$STAGE/bin" "$STAGE/libexec"
cp "$REPO_ROOT/target/release/te" "$STAGE/bin/te"
cp -R "$REPO_ROOT/apps/renderer/dist/terminal-effects-renderer" \
  "$STAGE/libexec/terminal-effects-renderer"
cp "$REPO_ROOT/LICENSE" "$REPO_ROOT/README.md" "$REPO_ROOT/THIRD_PARTY_NOTICES.md" "$STAGE/"
cp "$REPO_ROOT/apps/renderer/LICENSE.terminal-browser" "$STAGE/"
printf '%s\n' "$VERSION" > "$STAGE/VERSION"

TARBALL="$OUT/$PACKAGE.tar.gz"
tar -czf "$TARBALL" -C "$OUT" "$PACKAGE"
if command -v shasum >/dev/null 2>&1; then
  shasum -a 256 "$TARBALL" > "$TARBALL.sha256"
else
  sha256sum "$TARBALL" > "$TARBALL.sha256"
fi

du -h "$TARBALL"
