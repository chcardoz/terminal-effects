#!/bin/sh
set -eu

REPOSITORY="${TE_INSTALL_REPOSITORY:-chcardoz/terminal-effects}"
RELEASE_BASE="${TE_INSTALL_RELEASE_BASE:-https://github.com/$REPOSITORY/releases/latest/download}"

fail() {
  printf 'terminal-effects installer: %s\n' "$1" >&2
  exit 1
}

command -v curl >/dev/null 2>&1 || fail "curl is required"
command -v tar >/dev/null 2>&1 || fail "tar is required"

if [ -n "${TE_INSTALL_TARGET:-}" ]; then
  TARGET="$TE_INSTALL_TARGET"
else
  case "$(uname -s)-$(uname -m)" in
    Darwin-arm64) TARGET="darwin-arm64" ;;
    Linux-x86_64|Linux-amd64) TARGET="linux-x64" ;;
    Linux-aarch64|Linux-arm64) TARGET="linux-arm64" ;;
    Darwin-x86_64) fail "Intel macOS is not supported yet" ;;
    *) fail "unsupported platform: $(uname -s)-$(uname -m)" ;;
  esac
fi

case "$TARGET" in
  darwin-arm64|linux-x64|linux-arm64) ;;
  *) fail "unsupported release target: $TARGET" ;;
esac

ARCHIVE="terminal-effects-$TARGET.tar.gz"
TEMPORARY="$(mktemp -d "${TMPDIR:-/tmp}/terminal-effects-install.XXXXXX")"
trap 'rm -rf "$TEMPORARY"' EXIT HUP INT TERM

printf 'Downloading Terminal Effects for %s...\n' "$TARGET"
curl -fL --retry 3 --progress-bar "$RELEASE_BASE/$ARCHIVE" -o "$TEMPORARY/$ARCHIVE" ||
  fail "no stable release is available for $TARGET"
curl -fsSL --retry 3 "$RELEASE_BASE/$ARCHIVE.sha256" -o "$TEMPORARY/$ARCHIVE.sha256" ||
  fail "the release checksum is missing"

EXPECTED="$(awk 'NR == 1 { print $1 }' "$TEMPORARY/$ARCHIVE.sha256")"
case "$EXPECTED" in
  ''|*[!0-9a-fA-F]*) fail "the release checksum is invalid" ;;
esac
[ "${#EXPECTED}" -eq 64 ] || fail "the release checksum is invalid"

if command -v shasum >/dev/null 2>&1; then
  ACTUAL="$(shasum -a 256 "$TEMPORARY/$ARCHIVE" | awk '{ print $1 }')"
elif command -v sha256sum >/dev/null 2>&1; then
  ACTUAL="$(sha256sum "$TEMPORARY/$ARCHIVE" | awk '{ print $1 }')"
else
  fail "shasum or sha256sum is required to verify the download"
fi
[ "$ACTUAL" = "$EXPECTED" ] || fail "the downloaded archive failed checksum verification"

tar -xzf "$TEMPORARY/$ARCHIVE" -C "$TEMPORARY"
PACKAGE_DIR="$(find "$TEMPORARY" -mindepth 1 -maxdepth 1 -type d -name 'terminal-effects-*' | head -n 1)"
[ -n "$PACKAGE_DIR" ] || fail "the release archive has an invalid layout"
[ -f "$PACKAGE_DIR/VERSION" ] || fail "the release archive is missing VERSION"
[ -f "$PACKAGE_DIR/bin/te" ] || fail "the release archive is missing te"
[ -f "$PACKAGE_DIR/libexec/terminal-effects-renderer/bin/te-renderer" ] ||
  fail "the release archive is missing its renderer"

VERSION="$(tr -d '\r\n' < "$PACKAGE_DIR/VERSION")"
case "$VERSION" in
  ''|*[!0-9A-Za-z.-]*) fail "the release version is invalid" ;;
esac

DEFAULT_DATA_DIR="${XDG_DATA_HOME:-${HOME:?HOME is not set}/.local/share}/terminal-effects"
DATA_DIR="${TE_INSTALL_DATA_DIR:-$DEFAULT_DATA_DIR}"
BIN_DIR="${TE_INSTALL_BIN_DIR:-${HOME:?HOME is not set}/.local/bin}"
VERSIONS_DIR="$DATA_DIR/versions"
INSTALL_DIR="$VERSIONS_DIR/$VERSION"
COMMAND_LINK="$BIN_DIR/te"

mkdir -p "$VERSIONS_DIR" "$BIN_DIR"
if [ -e "$COMMAND_LINK" ] || [ -L "$COMMAND_LINK" ]; then
  [ -L "$COMMAND_LINK" ] || fail "$COMMAND_LINK already exists and is not a symlink"
  CURRENT_TARGET="$(readlink "$COMMAND_LINK")"
  case "$CURRENT_TARGET" in
    "$DATA_DIR"/versions/*/bin/te) ;;
    *) fail "$COMMAND_LINK belongs to another installation" ;;
  esac
fi
if [ -e "$INSTALL_DIR" ]; then
  [ -x "$INSTALL_DIR/bin/te" ] || fail "$INSTALL_DIR already exists but is not a valid installation"
  [ -x "$INSTALL_DIR/libexec/terminal-effects-renderer/bin/te-renderer" ] ||
    fail "$INSTALL_DIR already exists but is missing its renderer"
  ln -sfn "$INSTALL_DIR/bin/te" "$COMMAND_LINK"
  printf '\nTerminal Effects %s is already installed.\n' "$VERSION"
  printf '  executable: %s/te\n' "$BIN_DIR"
  exit 0
fi
mv "$PACKAGE_DIR" "$INSTALL_DIR"
ln -sfn "$INSTALL_DIR/bin/te" "$COMMAND_LINK"

printf '\nInstalled Terminal Effects %s.\n' "$VERSION"
printf '  executable: %s/te\n' "$BIN_DIR"

case ":${PATH:-}:" in
  *":$BIN_DIR:"*) ;;
  *) printf '\nAdd %s to PATH to run te from any directory.\n' "$BIN_DIR" ;;
esac

if ! command -v ffmpeg >/dev/null 2>&1 || ! command -v ffprobe >/dev/null 2>&1; then
  printf '\nFFmpeg is required for importing and exporting media.\n'
  case "$TARGET" in
    darwin-*) printf 'Install it with: brew install ffmpeg\n' ;;
    linux-*) printf 'Install the ffmpeg package with your Linux package manager.\n' ;;
  esac
fi
