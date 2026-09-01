#!/bin/bash
# Build the macOS disk image, from the .app Tauri has already built and signed.
#
#   tools/make_dmg.sh <path to .app> <output .dmg> [volume name]
#
# Why this exists rather than `tauri build --bundles dmg`. Tauri's dmg bundler
# makes a two-item window -- the app and a link to Applications -- and there is
# no way to put a third file in it. READ-ME-FIRST.rtf has to be in that window:
# it is where the reader learns, before they double-click anything, that macOS
# is about to refuse to open this app and what to do about it. A background
# picture that says "open READ ME FIRST" with no READ ME FIRST beside it would
# be worse than saying nothing.
#
# The .app is never modified here. It is copied in with `ditto` and nothing
# else is done to it. Tauri signs it ad-hoc during the build, and a signed
# bundle that is edited afterwards stops being "an app from an unidentified
# developer" and starts being "is damaged and can't be opened", which is a much
# worse thing for a reader to meet and would be entirely our doing.
#
# What is unverified here, and stays unverified until somebody opens this on a
# Mac: how any of it looks. The layout below is Finder scripting, it needs a
# window server, and it is allowed to fail -- if it does, the image still holds
# the app, the Applications link and the RTF, and this script says so in its
# last line. The words that matter are in the RTF either way.

set -euo pipefail

APP="${1:?usage: make_dmg.sh <app> <out.dmg> [volume name]}"
OUT="${2:?usage: make_dmg.sh <app> <out.dmg> [volume name]}"
VOL="${3:-The Pastor Bible}"

HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(dirname "$HERE")"
DMG_SRC="$ROOT/src-tauri/dmg"
BACKGROUND="$DMG_SRC/background.png"
README="$DMG_SRC/READ-ME-FIRST.rtf"

APP_NAME="$(basename "$APP")"
README_NAME="$(basename "$README")"

# These three must match tools/mac_dmg_background.py, which drew the gaps the
# icons sit in. Change one, run the other.
WIN_W=660
WIN_H=560
APP_X=170;  APP_Y=150
APPS_X=490; APPS_Y=150
DOC_X=330;  DOC_Y=300

for f in "$APP" "$BACKGROUND" "$README"; do
  [ -e "$f" ] || { echo "::error::$f is missing"; exit 1; }
done

STAGE="$(mktemp -d)"
MNT="$(mktemp -d)"
RW="$(mktemp -u).dmg"
trap 'hdiutil detach "$MNT" -force >/dev/null 2>&1 || true; rm -rf "$STAGE" "$MNT" "$RW"' EXIT

echo "staging $APP_NAME"
ditto "$APP" "$STAGE/$APP_NAME"
cp "$README" "$STAGE/$README_NAME"
mkdir -p "$STAGE/.background"
cp "$BACKGROUND" "$STAGE/.background/background.png"
ln -s /Applications "$STAGE/Applications"

# hdiutil can size the image itself, but it sizes it exactly, and Finder then
# has nowhere to write the .DS_Store that carries the whole layout. The slack
# is deliberate; the compressed image at the end does not carry it.
SIZE_MB=$(( $(du -sm "$STAGE" | cut -f1) + 150 ))
echo "creating a $SIZE_MB MB read-write image"
hdiutil create -volname "$VOL" -srcfolder "$STAGE" -fs HFS+ \
  -format UDRW -size "${SIZE_MB}m" -ov "$RW" >/dev/null

hdiutil attach "$RW" -readwrite -noverify -noautoopen -mountpoint "$MNT" >/dev/null
echo "mounted at $MNT:"
ls -la "$MNT"

LAYOUT=unknown
if osascript <<APPLESCRIPT
tell application "Finder"
  tell disk "$VOL"
    open
    set current view of container window to icon view
    set toolbar visible of container window to false
    set statusbar visible of container window to false
    set the bounds of container window to {200, 120, $((200 + WIN_W)), $((120 + WIN_H))}
    set opts to the icon view options of container window
    set arrangement of opts to not arranged
    set icon size of opts to 128
    set text size of opts to 13
    set background picture of opts to file ".background:background.png"
    set position of item "$APP_NAME" of container window to {$APP_X, $APP_Y}
    set position of item "Applications" of container window to {$APPS_X, $APPS_Y}
    set position of item "$README_NAME" of container window to {$DOC_X, $DOC_Y}
    close
    open
    update without registering applications
    delay 3
    close
  end tell
end tell
APPLESCRIPT
then
  LAYOUT=applied
else
  LAYOUT=failed
  echo "::warning::the Finder layout step failed. The disk image still holds" \
       "the app, the Applications link and $README_NAME; what it will not have" \
       "is the background picture and the icon positions."
fi

sync || true
hdiutil detach "$MNT" >/dev/null

# A .DS_Store is the only evidence, short of a person looking, that the layout
# was written at all. It is checked after the unmount because that is when
# Finder has finished flushing it.
hdiutil attach "$RW" -readonly -noverify -noautoopen -mountpoint "$MNT" >/dev/null
# `[ -f x ] && VAR=y` is an and-list, and under `set -e` a false test aborts
# the script -- which would turn "the layout did not happen" into "the build
# failed", the opposite of what this reporting is for.
DS=absent
if [ -f "$MNT/.DS_Store" ]; then DS="present, $(stat -f%z "$MNT/.DS_Store") bytes"; fi
BG=absent
if [ -f "$MNT/.background/background.png" ]; then BG=present; fi
hdiutil detach "$MNT" >/dev/null

rm -f "$OUT"
hdiutil convert "$RW" -format UDZO -imagekey zlib-level=9 -o "$OUT" >/dev/null

echo
echo "wrote $OUT ($(stat -f%z "$OUT") bytes)"
echo "  volume name       $VOL"
echo "  window            ${WIN_W}x${WIN_H}, icons 128"
echo "  contents          $APP_NAME, Applications, $README_NAME"
echo "  background file   $BG"
echo "  Finder layout     $LAYOUT (.DS_Store $DS)"
echo "  NOT VERIFIED      how any of this looks. Nobody has opened this window."
