#!/usr/bin/env bash
set -euo pipefail
dmg_path="${1:?usage: notarize-dmg.sh path-to-signed.dmg}"
test -f "$dmg_path"
: "${APPLE_API_KEY_PATH:?required}"
: "${APPLE_API_KEY:?required}"
: "${APPLE_API_ISSUER:?required}"
: "${RUNNER_TEMP:?required}"
test -f "$APPLE_API_KEY_PATH"
mount_point="$(mktemp -d "$RUNNER_TEMP/keepline-dmg.XXXXXX")"
notary_result="$(mktemp "$RUNNER_TEMP/keepline-notary.XXXXXX")"
mounted=0
cleanup() {
  if [ "$mounted" -eq 1 ]; then hdiutil detach "$mount_point" >/dev/null 2>&1 || true; fi
  rmdir "$mount_point" 2>/dev/null || true
  rm -f "$notary_result"
}
trap cleanup EXIT
hdiutil attach -nobrowse -readonly -mountpoint "$mount_point" "$dmg_path" >/dev/null
mounted=1
app_path="$(find "$mount_point" -maxdepth 1 -name '*.app' -print -quit)"
if [ -z "$app_path" ]; then
  echo "Signed app bundle is missing from the DMG" >&2
  exit 1
fi
codesign --verify --deep --strict --verbose=2 "$app_path"
spctl --assess --type execute --verbose=4 "$app_path"
xcrun stapler validate "$app_path"
hdiutil detach "$mount_point" >/dev/null
mounted=0
xcrun notarytool submit "$dmg_path" \
  --key "$APPLE_API_KEY_PATH" --key-id "$APPLE_API_KEY" --issuer "$APPLE_API_ISSUER" \
  --wait --output-format json > "$notary_result"
python3 - "$notary_result" <<'PYJSON'
import json
import sys
with open(sys.argv[1], encoding="utf-8") as result_file:
    result = json.load(result_file)
if result.get("status") != "Accepted":
    raise SystemExit("DMG notarization was not accepted")
PYJSON
stapled=0
for attempt in 1 2 3 4 5 6; do
  if xcrun stapler staple "$dmg_path"; then stapled=1; break; fi
  if [ "$attempt" -lt 6 ]; then
    echo "DMG ticket unavailable on attempt ${attempt}; retrying in 15s"
    sleep 15
  fi
done
if [ "$stapled" -ne 1 ]; then
  echo "Failed to staple the accepted notarization ticket onto the DMG" >&2
  exit 1
fi
xcrun stapler validate "$dmg_path"
spctl --assess --type open --context context:primary-signature --verbose=2 "$dmg_path"
