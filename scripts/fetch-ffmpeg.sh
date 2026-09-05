#!/usr/bin/env bash
# Downloads static ffmpeg/ffprobe builds and places them as Tauri sidecars in
# src-tauri/binaries/<tool>-<target-triple>. Run with no args for the host, or
# pass a target triple: aarch64-apple-darwin | x86_64-apple-darwin | x86_64-pc-windows-msvc
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DEST="$ROOT/src-tauri/binaries"
mkdir -p "$DEST"
TRIPLE="${1:-$(rustc -vV | sed -n 's/^host: //p')}"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

# ffmpeg 9.0.1 static builds. macOS: martin-riedl.de (includes AudioToolbox aac_at,
# libx264, libx265, libsvtav1, videotoolbox). Windows: BtbN GPL build.
MAC_ARM="https://ffmpeg.martin-riedl.de/download/macos/arm64/1787073674_9.0.1"
MAC_X64_BASE="https://ffmpeg.martin-riedl.de/download/macos/amd64"
WIN_ZIP="https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/ffmpeg-n9.0-latest-win64-gpl-9.0.zip"

case "$TRIPLE" in
  aarch64-apple-darwin)
    for t in ffmpeg ffprobe; do
      curl -fL --progress-bar "$MAC_ARM/$t.zip" -o "$TMP/$t.zip"
      unzip -qo "$TMP/$t.zip" -d "$TMP/$t"
      cp "$TMP/$t/$t" "$DEST/$t-$TRIPLE"; chmod +x "$DEST/$t-$TRIPLE"
    done ;;
  x86_64-apple-darwin)
    # newest release folder for amd64
    REL="$(curl -fsL https://ffmpeg.martin-riedl.de/ | grep -oE 'download/macos/amd64/[0-9]+_9\.[0-9.]+' | head -1)"
    [ -n "$REL" ] || { echo "could not find amd64 release build"; exit 1; }
    for t in ffmpeg ffprobe; do
      curl -fL --progress-bar "https://ffmpeg.martin-riedl.de/$REL/$t.zip" -o "$TMP/$t.zip"
      unzip -qo "$TMP/$t.zip" -d "$TMP/$t"
      cp "$TMP/$t/$t" "$DEST/$t-$TRIPLE"; chmod +x "$DEST/$t-$TRIPLE"
    done ;;
  x86_64-pc-windows-msvc)
    curl -fL --progress-bar "$WIN_ZIP" -o "$TMP/win.zip"
    unzip -qo "$TMP/win.zip" -d "$TMP/win"
    for t in ffmpeg ffprobe; do
      cp "$(find "$TMP/win" -name "$t.exe" | head -1)" "$DEST/$t-$TRIPLE.exe"
    done ;;
  *) echo "unsupported triple: $TRIPLE"; exit 1 ;;
esac

# Verify the encoders the app relies on are present (skipped for cross-target binaries).
BIN="$DEST/ffmpeg-$TRIPLE"; [ -x "$BIN" ] || BIN="$BIN.exe"
if [ "$TRIPLE" = "$(rustc -vV | sed -n 's/^host: //p')" ]; then
  ENC="$("$BIN" -hide_banner -encoders 2>/dev/null)"
  for e in libx264 libx265 libsvtav1; do echo "$ENC" | grep -q " $e " || { echo "missing encoder $e"; exit 1; }; done
  case "$TRIPLE" in *apple*) echo "$ENC" | grep -q " aac_at " || echo "warning: aac_at (Apple AAC) missing";; esac
fi
echo "sidecars ready in $DEST:"; ls -la "$DEST"
