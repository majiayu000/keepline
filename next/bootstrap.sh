#!/bin/sh
set -eu
cd "$(dirname "$0")"
mkdir -p .deps
checkout() {
  name=$1 url=$2 rev=$3
  if [ ! -e ".deps/$name" ]; then
    git init ".deps/$name"
    git -C ".deps/$name" remote add origin "$url"
    git -C ".deps/$name" fetch --depth 1 origin "$rev"
    git -C ".deps/$name" checkout --detach FETCH_HEAD
  fi
  test "$(git -C ".deps/$name" rev-parse HEAD)" = "$rev" || {
    echo "$name has a different revision; refusing to replace it" >&2; exit 1;
  }
}
checkout zed https://github.com/zed-industries/zed.git 2a857f5f2ec368745c1af5209d49836968fe75b8
checkout libghostty-rs https://github.com/Uzaaft/libghostty-rs.git 0ef80413de1326685743378efbfa06d849e041a7
for patch_path in "$(pwd)/patches/gpui-idle.patch" "$(pwd)/patches/gpui-glyph-dilation.patch" "$(pwd)/patches/gpui-retained-backbuffer.patch"; do
  if ! git -C .deps/zed apply --reverse --check "$patch_path" 2>/dev/null; then
    git -C .deps/zed apply --check "$patch_path"
    git -C .deps/zed apply "$patch_path"
  fi
done
"${ZIG:-zig}" version
printf '%s\n' 'Dependencies ready. Build with Xcode available and Zig 0.16.x.'
