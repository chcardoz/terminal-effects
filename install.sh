#!/bin/sh
set -eu

# Keep this file at the repository root: its raw GitHub URL is the stable public
# installation entrypoint. The installer implementation lives with the rest of
# the release tooling.
REPOSITORY="${TE_INSTALL_REPOSITORY:-chcardoz/terminal-effects}"
REF="${TE_INSTALL_REF:-main}"
INSTALLER_URL="${TE_INSTALLER_URL:-https://raw.githubusercontent.com/$REPOSITORY/$REF/tooling/installer/install.sh}"

fail() {
  printf 'terminal-effects installer bootstrap: %s\n' "$1" >&2
  exit 1
}

command -v curl >/dev/null 2>&1 || fail "curl is required"

BOOTSTRAP_DIR="$(mktemp -d "${TMPDIR:-/tmp}/terminal-effects-bootstrap.XXXXXX")"
trap 'rm -rf "$BOOTSTRAP_DIR"' EXIT HUP INT TERM
INSTALLER="$BOOTSTRAP_DIR/install.sh"

curl -fsSL --retry 3 "$INSTALLER_URL" -o "$INSTALLER" ||
  fail "could not download the installer implementation"

sh "$INSTALLER" "$@"
