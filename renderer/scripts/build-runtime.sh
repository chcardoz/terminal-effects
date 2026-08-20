#!/bin/bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
VERSION="${1:-dev}"
OUT="$ROOT/dist"
STAGE="$OUT/terminal-effects-renderer"

case "$(uname -s)-$(uname -m)" in
  Darwin-arm64) TARGET=darwin-arm64 ;;
  Linux-x86_64|Linux-amd64) TARGET=linux-x64 ;;
  Linux-aarch64|Linux-arm64) TARGET=linux-arm64 ;;
  *) echo "unsupported renderer build host: $(uname -s)-$(uname -m)" >&2; exit 1 ;;
esac

rm -rf "$OUT"
mkdir -p "$STAGE"/{assets/fonts,bin,browser/dist,browser/native,electron,launcher/dist}

(cd "$ROOT/engine" && cargo build -p pixel-node --release)
if [ "$TARGET" = darwin-arm64 ]; then
  NATIVE_LIB=libpixel_node.dylib
else
  NATIVE_LIB=libpixel_node.so
fi
cp "${CARGO_TARGET_DIR:-$ROOT/engine/target}/release/$NATIVE_LIB" "$STAGE/browser/native/pixel.node"

if [ "$TARGET" = darwin-arm64 ]; then
  swiftc -O -target arm64-apple-macos11 \
    "$ROOT/engine/crates/pixel-core/native-scroll-helper.swift" \
    -o "$STAGE/bin/native-scroll-helper"
  codesign --force --sign - --timestamp=none "$STAGE/bin/native-scroll-helper" 2>/dev/null || true
fi

"$ROOT/scripts/bundle.sh" "$ROOT/launcher/src/main.ts" "$STAGE/launcher/dist/main.js"
"$ROOT/scripts/bundle.sh" "$ROOT/browser/src/main.tsx" "$STAGE/browser/dist/main.js"
rm -f "$STAGE/launcher/dist/main.js.map" "$STAGE/browser/dist/main.js.map"
cp "$ROOT/assets/fonts/JetBrainsMono-Regular.ttf" "$STAGE/assets/fonts/"

ELECTRON_DIST="$(node -e 'const p=require("path");console.log(p.join(p.dirname(require.resolve("electron/package.json",{paths:[process.argv[1]]})),"dist"))' "$ROOT/browser")"
if [ ! -f "$ELECTRON_DIST/.zenbu-electron-sha256" ]; then
  echo "renderer requires the verified patched Electron runtime; run pnpm install" >&2
  exit 1
fi

if [ "$TARGET" = darwin-arm64 ]; then
  APP="$STAGE/electron/Terminal Effects Renderer.app"
  ditto "$ELECTRON_DIST/Electron.app" "$APP"
  mv "$APP/Contents/MacOS/Electron" "$APP/Contents/MacOS/Terminal Effects Renderer"
  /usr/libexec/PlistBuddy \
    -c "Set :CFBundleExecutable Terminal Effects Renderer" \
    -c "Set :CFBundleName Terminal Effects Renderer" \
    -c "Set :CFBundleDisplayName Terminal Effects Renderer" \
    -c "Set :CFBundleIdentifier dev.terminal-effects.renderer" \
    "$APP/Contents/Info.plist" >/dev/null
  codesign --force --deep --sign - --timestamp=none "$APP" 2>/dev/null
  ELECTRON_EXE="electron/Terminal Effects Renderer.app/Contents/MacOS/Terminal Effects Renderer"
  NATIVE_SCROLL='export NATIVE_SCROLL_HELPER="${NATIVE_SCROLL_HELPER:-$ROOT/bin/native-scroll-helper}"'
else
  cp -a "$ELECTRON_DIST/." "$STAGE/electron/"
  ELECTRON_EXE="electron/electron"
  NATIVE_SCROLL=""
fi

cat > "$STAGE/bin/te-renderer" <<EOF
#!/bin/sh
ROOT="\$(CDPATH= cd -- "\$(dirname -- "\$0")/.." && pwd -P)"
export TE_RENDERER_DIST_ROOT="\$ROOT"
export ELECTRON_RUN_AS_NODE=1
$NATIVE_SCROLL
exec "\$ROOT/$ELECTRON_EXE" "\$ROOT/launcher/dist/main.js" "\$@"
EOF
chmod +x "$STAGE/bin/te-renderer"
echo "$VERSION" > "$STAGE/VERSION"

TARBALL="$OUT/terminal-effects-renderer-$VERSION-$TARGET.tar.gz"
tar -czf "$TARBALL" -C "$OUT" terminal-effects-renderer

du -h "$TARBALL"
