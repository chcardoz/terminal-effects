#!/bin/sh
set -eu

ROOT="$(cd "$(dirname "$0")/../.." && pwd -P)"
TEST_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/terminal-effects-installer-test.XXXXXX")"
trap 'rm -rf "$TEST_ROOT"' EXIT HUP INT TERM

TARGET="darwin-arm64"
VERSION="9.8.7-test.1"
PACKAGE="terminal-effects-$VERSION-$TARGET"
RELEASES="$TEST_ROOT/releases"
STAGE="$TEST_ROOT/$PACKAGE"
mkdir -p "$RELEASES" "$STAGE/bin" "$STAGE/libexec/terminal-effects-renderer/bin"

printf '%s\n' "$VERSION" > "$STAGE/VERSION"
printf '#!/bin/sh\nprintf "te %s\\n"\n' "$VERSION" > "$STAGE/bin/te"
printf '#!/bin/sh\nexit 0\n' > "$STAGE/libexec/terminal-effects-renderer/bin/te-renderer"
chmod +x "$STAGE/bin/te" "$STAGE/libexec/terminal-effects-renderer/bin/te-renderer"

ARCHIVE="$RELEASES/terminal-effects-$TARGET.tar.gz"
tar -czf "$ARCHIVE" -C "$TEST_ROOT" "$PACKAGE"
if command -v shasum >/dev/null 2>&1; then
  shasum -a 256 "$ARCHIVE" > "$ARCHIVE.sha256"
else
  sha256sum "$ARCHIVE" > "$ARCHIVE.sha256"
fi

TE_INSTALL_TARGET="$TARGET" \
TE_INSTALL_RELEASE_BASE="file://$RELEASES" \
TE_INSTALL_DATA_DIR="$TEST_ROOT/data" \
TE_INSTALL_BIN_DIR="$TEST_ROOT/bin" \
  "$ROOT/install.sh" >/dev/null

[ -L "$TEST_ROOT/bin/te" ]
[ "$("$TEST_ROOT/bin/te" --version)" = "te $VERSION" ]
[ -x "$TEST_ROOT/data/versions/$VERSION/libexec/terminal-effects-renderer/bin/te-renderer" ]
printf 'installer test passed\n'
