#!/usr/bin/env bash
#
# Automated store-metadata screenshot pipeline for Bandao.
#
# Boots simulators for each iOS device class App Store cares about, runs
# the integration_test in `integration_test/screenshot_test.dart`, and
# writes the captured PNGs straight into `store_metadata/ios/screenshots/`.
#
# Test credentials are passed via flags or environment variables; they
# never enter the repo. The simplest invocation:
#
#   ./scripts/take_screenshots.sh \
#     --org-code   ABC123 \
#     --username   test@example.com \
#     --password   yourPassword \
#     --api        https://bandao-api.ccmos.tw   # optional, default shown
#
# Or via env:  BANDAO_TEST_ORG_CODE / _USERNAME / _PASSWORD / _API.
#
# Skips simulators that aren't available on this machine. To rebuild the
# device class list, see `xcrun simctl list devices`.
#
# For Android, see the sibling script `take_android_screenshots.sh` —
# same Dart test, different device-class discovery + boot path.

set -euo pipefail

# ── Argument parsing ────────────────────────────────────────────────────
ORG_CODE="${BANDAO_TEST_ORG_CODE:-}"
USERNAME="${BANDAO_TEST_USERNAME:-}"
PASSWORD="${BANDAO_TEST_PASSWORD:-}"
API_URL="${BANDAO_TEST_API:-https://bandao-api.ccmos.tw}"

usage() {
  cat <<'USAGE'
Usage: take_screenshots.sh [options]

Required (also accept env vars: BANDAO_TEST_ORG_CODE / _USERNAME / _PASSWORD):
  --org-code  CODE     Org invite code for the test account.
  --username  NAME     AppUser username.
  --password  PASS     AppUser password.

Optional:
  --api       URL      API base URL (default https://bandao-api.ccmos.tw).
  -h, --help           Print this help.

Output:
  store_metadata/ios/screenshots/iphone_6.7/{01_login,02_home,03_history,04_trajectory}.png
  store_metadata/ios/screenshots/ipad_12.9/{01_login,02_home,03_history,04_trajectory}.png
USAGE
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --org-code)  ORG_CODE="$2"; shift 2 ;;
    --username)  USERNAME="$2"; shift 2 ;;
    --password)  PASSWORD="$2"; shift 2 ;;
    --api)       API_URL="$2"; shift 2 ;;
    -h|--help)   usage; exit 0 ;;
    *) echo "Unknown option: $1" >&2; usage; exit 2 ;;
  esac
done

if [[ -z "$ORG_CODE" || -z "$USERNAME" || -z "$PASSWORD" ]]; then
  echo "Missing required credentials." >&2
  usage
  exit 2
fi

# ── Setup ───────────────────────────────────────────────────────────────
APP_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$APP_ROOT"

# Ensure deps + codegen are fresh — drift_dev needs to run before
# integration_test compiles the queue DB.
echo "──▶ flutter pub get"
flutter pub get >/dev/null
echo "──▶ build_runner codegen (drift)"
dart run build_runner build --delete-conflicting-outputs >/dev/null 2>&1 || true

# ── iOS device classes ─────────────────────────────────────────────────
# Each entry: "device_class:simulator_name_substring"
# device_class becomes the screenshots/ subdirectory App Store expects.
# simulator_name_substring is grep'd against `xcrun simctl list devices`.
DEVICES=(
  "iphone_6.7:iPhone 16 Pro Max"
  "ipad_12.9:iPad Pro 13-inch (M4)"
)

# Fallback search list per device class — first match wins. Names go from
# newest to oldest so we prefer the latest-generation simulator. Each entry
# must match an exact device name from `xcrun simctl list devices`.
declare -a IPHONE_67_NAMES=(
  "iPhone 17 Pro Max"
  "iPhone 17 Plus"
  "iPhone 16 Pro Max"
  "iPhone 16 Plus"
  "iPhone 15 Pro Max"
  "iPhone 15 Plus"
  "iPhone 14 Pro Max"
  "iPhone 14 Plus"
)
declare -a IPAD_129_NAMES=(
  "iPad Pro 13-inch (M4)"
  "iPad Pro 12.9-inch (6th generation)"
  "iPad Pro 12.9-inch (5th generation)"
  "iPad Pro 12.9-inch"
)

# Find the first installed simulator matching one of the candidate names.
# Echoes the matched name on success; returns non-zero on no match.
#
# Stays bash-3.2 compatible (macOS default `/bin/bash`) — no namerefs.
resolve_simulator() {
  local class="$1"
  local -a names
  case "$class" in
    iphone_6.7) names=("${IPHONE_67_NAMES[@]}") ;;
    ipad_12.9)  names=("${IPAD_129_NAMES[@]}") ;;
    *) return 1 ;;
  esac

  local available
  available="$(xcrun simctl list devices available)"

  local name
  for name in "${names[@]}"; do
    # Match `<name> (UDID) (...)` exactly — the parenthesis after the name
    # separates the device name from the UDID. This avoids "iPhone" matching
    # "iPhone 17", which `flutter drive` then treats as ambiguous.
    if printf '%s\n' "$available" | grep -F " $name (" >/dev/null; then
      echo "$name"
      return 0
    fi
  done
  return 1
}

# ── Capture loop ───────────────────────────────────────────────────────
for entry in "${DEVICES[@]}"; do
  CLASS="${entry%%:*}"
  if ! SIM_NAME="$(resolve_simulator "$CLASS")"; then
    echo "──✗ skip $CLASS — no matching simulator installed." >&2
    case "$CLASS" in
      iphone_6.7)
        echo "    Need a Pro Max / Plus class iPhone simulator." >&2
        echo "    Open Xcode → Window → Devices and Simulators → Simulators tab" >&2
        echo "    → '+' → pick e.g. 'iPhone 17 Pro Max' or 'iPhone 16 Pro Max'." >&2
        echo "    Or: xcodebuild -downloadPlatform iOS" >&2
        ;;
      ipad_12.9)
        echo "    Need an iPad Pro 12.9\"+ simulator (M4 / 6th gen / 5th gen)." >&2
        echo "    Open Xcode → Window → Devices and Simulators → Simulators tab" >&2
        echo "    → '+' → pick e.g. 'iPad Pro 13-inch (M4)'." >&2
        ;;
    esac
    echo >&2
    continue
  fi

  OUT_DIR="$APP_ROOT/store_metadata/ios/screenshots/$CLASS"
  mkdir -p "$OUT_DIR"

  echo "──▶ $CLASS via \"$SIM_NAME\""
  echo "    output → $OUT_DIR"

  # Boot simulator (idempotent — `simctl boot` is fine on already-booted).
  SIM_UDID="$(xcrun simctl list devices available | grep -F " $SIM_NAME (" \
              | head -1 | sed -E 's/.*\(([-A-F0-9]+)\).*/\1/')"
  xcrun simctl boot "$SIM_UDID" 2>/dev/null || true
  open -a Simulator

  # Run the integration test. flutter drive talks to the booted simulator
  # and the driver process here on the host writes PNGs to OUT_DIR.
  #
  # On iOS simulator, only `--debug` is supported (release/profile are
  # rejected with "release/profile builds are only supported for physical
  # devices"). The DEBUG ribbon doesn't appear because BandaoApp sets
  # `debugShowCheckedModeBanner: false`, so debug-mode screenshots are
  # still App-Store-clean.
  #
  # SCREENSHOT_OUT_DIR is exported as an env var (not --dart-define)
  # because flutter drive's defines only reach the device-side test;
  # the driver process running on the host reads via Platform.environment.
  SCREENSHOT_OUT_DIR="$OUT_DIR" flutter drive \
    --driver=test_driver/integration_driver.dart \
    --target=integration_test/screenshot_test.dart \
    -d "$SIM_UDID" \
    --debug \
    --dart-define="API_BASE_URL=$API_URL" \
    --dart-define="TEST_ORG_CODE=$ORG_CODE" \
    --dart-define="TEST_USERNAME=$USERNAME" \
    --dart-define="TEST_PASSWORD=$PASSWORD"

  echo "──▶ $CLASS done"
done

echo
echo "All screenshots written under store_metadata/ios/screenshots/."
ls -1 "$APP_ROOT/store_metadata/ios/screenshots/" | while read -r d; do
  count=$(find "$APP_ROOT/store_metadata/ios/screenshots/$d" -name '*.png' | wc -l | tr -d ' ')
  echo "  $d/  →  $count PNG(s)"
done
