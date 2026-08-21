#!/bin/bash
set -euo pipefail

BINARY="$1"

[ "$(uname -s)" = Linux ] || exit 0
[ -z "${TE_RENDERER_SKIP_APPARMOR:-}" ] || exit 0
[ "$(cat /proc/sys/kernel/apparmor_restrict_unprivileged_userns 2>/dev/null || true)" = 1 ] || exit 0

if [ ! -x "$BINARY" ]; then
  echo "no Terminal Effects renderer at $BINARY" >&2
  exit 1
fi
BINARY="$(readlink -f "$BINARY")"

[ ! -u "$(dirname "$BINARY")/chrome-sandbox" ] || exit 0

NAME="terminal-effects-renderer-$(printf '%s' "$BINARY" | sha256sum | cut -c1-12)"
PROFILE="/etc/apparmor.d/$NAME"

if [ -f /etc/apparmor.d/abi/5.0 ]; then
  ABI=5.0
elif [ -f /etc/apparmor.d/abi/4.0 ]; then
  ABI=4.0
else
  echo "no supported AppArmor ABI is installed" >&2
  exit 1
fi

WANTED="abi <abi/${ABI}>,

include <tunables/global>

@{exec_path} = $BINARY
profile $NAME @{exec_path} flags=(unconfined) {
  userns,
  @{exec_path} mr,

  include if exists <local/$NAME>
}"

[ "$(cat "$PROFILE" 2>/dev/null || true)" != "$WANTED" ] || exit 0

echo "Terminal Effects needs an AppArmor profile for its bundled Chromium renderer."
TMP="$(mktemp)"
trap 'rm -f "$TMP"' EXIT
printf '%s\n' "$WANTED" > "$TMP"

SUDO=sudo
[ "$(id -u)" != 0 ] || SUDO=""
if [ -n "$SUDO" ] && ! command -v sudo >/dev/null 2>&1; then
  echo "sudo is unavailable; install $TMP as $PROFILE and reload AppArmor" >&2
  trap - EXIT
  exit 1
fi

$SUDO install -m 0644 -o root -g root "$TMP" "$PROFILE"
if ! $SUDO apparmor_parser -r "$PROFILE"; then
  $SUDO rm -f "$PROFILE"
  exit 1
fi
