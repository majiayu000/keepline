#!/bin/sh
# Rerun the Ghostty vs Keepline terminal rendering comparison end to end.
#   ./run.sh            regenerate synthetic fixtures, render both sides, compare
#   ./run.sh --record   also re-record codex / vim / lazygit (fixtures/*.vt) first
# Outputs: out/ghostty/*.png, out/keepline/*.png, out/compare/*.png, out/report.{json,md}
set -eu
here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/../.." && pwd)
cd "$here"
export ZIG=${ZIG:-/Users/lifcc/Desktop/code/AI/tools/keepline/.claude/worktrees/terminal-eval-04a203/eval/terminal/vendor/zig/zig}
export DEVELOPER_DIR=${DEVELOPER_DIR:-/Applications/Xcode.app/Contents/Developer}
export LIBGHOSTTY_VT_SYS_OPTIMIZE=${LIBGHOSTTY_VT_SYS_OPTIMIZE:-ReleaseFast}
cols=${REF_COLS:-100} rows=${REF_ROWS:-30}

python3 gen_fixtures.py
[ "${1:-}" = "--record" ] && python3 record.py
rm -rf out && mkdir -p out
# Builds only the example binary: target/debug/keepline-app (possibly running) is not rewritten.
(cd "$root" && cargo build -q -p keepline-app --features visual-test --example terminal_reference)
"$root/target/debug/examples/terminal_reference" --cols "$cols" --rows "$rows" --out out/keepline fixtures/*.vt
for f in fixtures/*.vt; do
  ./ghostty_shot.sh "$f" "out/ghostty/$(basename "$f" .vt).png"
done
uv run --quiet compare.py --cols "$cols" --rows "$rows"
