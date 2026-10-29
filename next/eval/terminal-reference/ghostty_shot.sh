#!/bin/sh
# Show one raw VT fixture in a fresh, isolated Ghostty instance and capture its window.
# Usage: ghostty_shot.sh <fixture.vt> <out.png>
# Env: GHOSTTY (binary), REF_FONT (default: Ghostty's built-in JetBrains Mono), REF_SIZE (13),
#      REF_COLS/REF_ROWS (100x30).
# Needs Screen Recording permission for the calling app (screencapture -l).
set -eu
here=$(cd "$(dirname "$0")" && pwd)
fixture=$(cd "$(dirname "$1")" && pwd)/$(basename "$1")
out=$2
GHOSTTY=${GHOSTTY:-/Applications/Ghostty.app/Contents/MacOS/ghostty}
work=${TMPDIR:-/tmp}/keepline-terminal-reference
mkdir -p "$work" "$(dirname "$out")"
winid=$work/window_id
[ -x "$winid" ] && [ "$winid" -nt "$here/window_id.swift" ] || swiftc -O "$here/window_id.swift" -o "$winid"

# -opost: the fixture already carries CR/LF; -echo: replies to app queries stay invisible.
cat >"$work/show.sh" <<EOF
#!/bin/sh
stty -opost -echo -icanon
cat '$fixture'
exec sleep 600
EOF
chmod +x "$work/show.sh"
# Colors mirror Keepline's pane defaults (theme.rs PANE / BODY) so diffs show rendering, not theme.
cat >"$work/ghostty.conf" <<EOF
${REF_FONT:+font-family = $REF_FONT}
font-size = ${REF_SIZE:-13}
font-thicken = false
font-feature = -calt
font-feature = -liga
window-width = ${REF_COLS:-100}
window-height = ${REF_ROWS:-30}
window-padding-x = 0
window-padding-y = 0
window-padding-balance = false
window-save-state = never
window-step-resize = false
macos-titlebar-style = hidden
background = 101217
foreground = d7dae0
cursor-color = d7dae0
cursor-style = block
cursor-style-blink = false
shell-integration = none
confirm-close-surface = false
quit-after-last-window-closed = true
initial-window = true
command = $work/show.sh
EOF
# Launch through LaunchServices: a bare binary launch never becomes the key window, and an
# unfocused Ghostty draws every cursor as a hollow block, hiding DECSCUSR shapes.
app=${GHOSTTY%/Contents/MacOS/*}
open -na "$app" --args --config-default-files=false --config-file="$work/ghostty.conf"
pid=""
for _ in $(seq 40); do
  pid=$(pgrep -n -f "config-file=$work/ghostty.conf" || true)
  [ -n "$pid" ] && break
  sleep 0.25
done
[ -n "$pid" ] || { echo "ghostty did not start" >&2; exit 1; }
# Wait for exit, so the next shot's window is not de-focused when this instance quits.
trap 'kill $pid 2>/dev/null; while kill -0 $pid 2>/dev/null; do sleep 0.1; done' EXIT
geom=""
for _ in $(seq 40); do
  geom=$("$winid" "$pid" 2>/dev/null || true)
  [ -n "$geom" ] && break
  sleep 0.25
done
[ -n "$geom" ] || { echo "ghostty window not found" >&2; exit 1; }
osascript -e "tell application \"System Events\" to set frontmost of (first process whose unix id is $pid) to true" >/dev/null 2>&1 || true
sleep 1.5
id=${geom%% *}
screencapture -x -o -l "$id" "$out"
# screencapture tags pixels with the display profile; Keepline's render_to_image is raw sRGB.
sips -m "/System/Library/ColorSync/Profiles/sRGB Profile.icc" "$out" >/dev/null
echo "ghostty $(basename "$fixture" .vt): window=$geom -> $out"
