#!/usr/bin/env bash
#
# Automated store-metadata screenshot pipeline for Bandao — Android variant.
#
# Boots Android emulators for each device class Play Store cares about,
# runs the integration_test in `integration_test/screenshot_test.dart`,
# and writes the captured PNGs straight into
# `store_metadata/android/images/{phone,tablet}-screenshots/`.
#
# Test credentials are passed via flags or environment variables; they
# never enter the repo. The simplest invocation:
#
#   ./scripts/take_android_screenshots.sh \
#     --org-code   ABC123 \
#     --username   test@example.com \
#     --password   yourPassword \
#     --api        https://bandao-api.ccmos.tw   # optional
#
# Or via env:  BANDAO_TEST_ORG_CODE / _USERNAME / _PASSWORD / _API.
#
# Captures the same three baseline screens as iOS (01_login / 02_home /
# 03_history). The Play Data Safety sticky-notification proof shot is
# captured manually on a real device — that flow needs to grant runtime
# location permission and pull down the status bar, which is outside
# integration_test's reach.
#
# Prereqs:
#   - $ANDROID_HOME (or $ANDROID_SDK_ROOT) pointing at the Android SDK
#     (Android Studio → Preferences → Languages & Frameworks → Android SDK).
#   - At least one phone-class AVD created; tablet-class AVD optional.
#     Create via Android Studio → Tools → Device Manager → Create device.

set -euo pipefail

# ── Argument parsing ────────────────────────────────────────────────────
ORG_CODE="${BANDAO_TEST_ORG_CODE:-}"
USERNAME="${BANDAO_TEST_USERNAME:-}"
PASSWORD="${BANDAO_TEST_PASSWORD:-}"
API_URL="${BANDAO_TEST_API:-https://bandao-api.ccmos.tw}"
PHONE_AVD_OVERRIDE=""
TABLET_AVD_OVERRIDE=""

usage() {
  cat <<'USAGE'
Usage: take_android_screenshots.sh [options]

Required (also accept env vars: BANDAO_TEST_ORG_CODE / _USERNAME / _PASSWORD):
  --org-code    CODE   Org invite code for the test account.
  --username    NAME   AppUser username.
  --password    PASS   AppUser password.

Optional:
  --api         URL    API base URL (default https://bandao-api.ccmos.tw).
  --phone-avd   NAME   Override phone-class AVD discovery with an exact AVD name.
  --tablet-avd  NAME   Override tablet-class AVD discovery (skips tablet if unset
                       and no fallback match is found — tablet is optional).
  -h, --help           Print this help.

Output:
  store_metadata/android/images/phone-screenshots/{01_login,02_home,03_history}.png
  store_metadata/android/images/tablet-screenshots/{01_login,02_home,03_history}.png
USAGE
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --org-code)   ORG_CODE="$2"; shift 2 ;;
    --username)   USERNAME="$2"; shift 2 ;;
    --password)   PASSWORD="$2"; shift 2 ;;
    --api)        API_URL="$2"; shift 2 ;;
    --phone-avd)  PHONE_AVD_OVERRIDE="$2"; shift 2 ;;
    --tablet-avd) TABLET_AVD_OVERRIDE="$2"; shift 2 ;;
    -h|--help)    usage; exit 0 ;;
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

ANDROID_HOME="${ANDROID_HOME:-${ANDROID_SDK_ROOT:-$HOME/Library/Android/sdk}}"
if [[ ! -d "$ANDROID_HOME" ]]; then
  echo "Cannot find Android SDK at $ANDROID_HOME — set ANDROID_HOME." >&2
  exit 1
fi
ADB="${ANDROID_HOME}/platform-tools/adb"
EMULATOR="${ANDROID_HOME}/emulator/emulator"
if [[ ! -x "$ADB" ]] || [[ ! -x "$EMULATOR" ]]; then
  echo "Missing adb or emulator under $ANDROID_HOME." >&2
  echo "Install via Android Studio → SDK Manager → SDK Tools." >&2
  exit 1
fi

echo "──▶ flutter pub get"
flutter pub get >/dev/null
echo "──▶ build_runner codegen (drift)"
dart run build_runner build --delete-conflicting-outputs >/dev/null 2>&1 || true

# ── AVD classes ─────────────────────────────────────────────────────────
# Order matters — first installed AVD per class wins, newest-first.
DEVICES=(
  "phone-screenshots"
  "tablet-screenshots"
)
declare -a PHONE_AVD_NAMES=(
  "Pixel_8_Pro"
  "Pixel_8"
  "Pixel_7_Pro"
  "Pixel_7"
  "Pixel_6_Pro"
  "Pixel_6"
  "Pixel_5"
  "Pixel_4"
  "Medium_Phone"  # Android Studio default — matches Medium_Phone_API_*
  "Small_Phone"
  "Large_Phone"
)
declare -a TABLET_AVD_NAMES=(
  "Pixel_Tablet"
  "Medium_Tablet"  # Android Studio default — matches Medium_Tablet_API_*
  "Nexus_9"
  "Pixel_C"
)

# Substring-match an AVD entry against installed AVDs and echo the first
# match's full name (e.g. "Medium_Phone" → "Medium_Phone_API_36.0"). The
# fallback list is ordered newest-first, so earlier list entries win.
resolve_avd() {
  local class="$1"
  local -a names
  local override=""
  case "$class" in
    phone-screenshots)
      names=("${PHONE_AVD_NAMES[@]}")
      override="$PHONE_AVD_OVERRIDE"
      ;;
    tablet-screenshots)
      names=("${TABLET_AVD_NAMES[@]}")
      override="$TABLET_AVD_OVERRIDE"
      ;;
    *) return 1 ;;
  esac

  local available
  available="$("$EMULATOR" -list-avds 2>/dev/null)"

  if [[ -n "$override" ]]; then
    if printf '%s\n' "$available" | grep -Fx "$override" >/dev/null; then
      echo "$override"
      return 0
    fi
    echo "──✗ override AVD \"$override\" not found in 'emulator -list-avds'" >&2
    return 1
  fi

  local name matched
  for name in "${names[@]}"; do
    matched="$(printf '%s\n' "$available" | grep -F "$name" | head -1 || true)"
    if [[ -n "$matched" ]]; then
      echo "$matched"
      return 0
    fi
  done
  return 1
}

wait_for_boot() {
  local serial="$1"
  echo "    waiting for $serial to finish booting (up to ~3 min)..."
  "$ADB" -s "$serial" wait-for-device
  local ready=""
  for _ in $(seq 1 90); do
    ready="$("$ADB" -s "$serial" shell getprop sys.boot_completed 2>/dev/null | tr -d '\r')"
    if [[ "$ready" == "1" ]]; then
      return 0
    fi
    sleep 2
  done
  echo "    timed out waiting for $serial to boot." >&2
  return 1
}

# ── Capture loop ───────────────────────────────────────────────────────
for CLASS in "${DEVICES[@]}"; do
  if ! AVD_NAME="$(resolve_avd "$CLASS")"; then
    echo "──✗ skip $CLASS — no matching AVD installed." >&2
    case "$CLASS" in
      phone-screenshots)
        echo "    Need a phone AVD (Pixel 4/5/6/7/8 family)." >&2
        echo "    Android Studio → Device Manager → Create device → Phone." >&2
        ;;
      tablet-screenshots)
        echo "    Need a tablet AVD (Pixel Tablet / Nexus 9 / Pixel C) — optional." >&2
        ;;
    esac
    echo >&2
    continue
  fi

  OUT_DIR="$APP_ROOT/store_metadata/android/images/$CLASS"
  mkdir -p "$OUT_DIR"

  echo "──▶ $CLASS via AVD \"$AVD_NAME\""
  echo "    output → $OUT_DIR"

  # Snapshot existing emulator serials so we can isolate the new one even if
  # the operator already has other emulators running.
  EXISTING="$("$ADB" devices | awk '/^emulator-[0-9]+/ {print $1}' | sort)"

  # Log emulator output to a tmp file so first-boot errors are recoverable
  # — `/dev/null` swallowed real diagnostics on the first run.
  EMU_LOG="/tmp/bandao-emulator-${CLASS}.log"
  : >"$EMU_LOG"
  "$EMULATOR" -avd "$AVD_NAME" -no-snapshot-save -no-boot-anim >"$EMU_LOG" 2>&1 &
  EMULATOR_PID=$!
  echo "    emulator log → $EMU_LOG"

  # Cold-boot an API 36 AVD on M-series Macs takes 40-90s before adb even
  # sees the device. Poll for up to ~3 min before giving up.
  SERIAL=""
  for _ in $(seq 1 90); do
    sleep 2
    CURRENT="$("$ADB" devices | awk '/^emulator-[0-9]+/ {print $1}' | sort)"
    SERIAL="$(comm -13 <(printf '%s\n' "$EXISTING") <(printf '%s\n' "$CURRENT") | head -1)"
    [[ -n "$SERIAL" ]] && break
  done
  if [[ -z "$SERIAL" ]]; then
    echo "    emulator $AVD_NAME never appeared in adb devices" >&2
    echo "    last 20 lines of $EMU_LOG:" >&2
    tail -20 "$EMU_LOG" >&2 || true
    kill "$EMULATOR_PID" 2>/dev/null || true
    continue
  fi

  if ! wait_for_boot "$SERIAL"; then
    "$ADB" -s "$SERIAL" emu kill 2>/dev/null || true
    kill "$EMULATOR_PID" 2>/dev/null || true
    continue
  fi

  # Capture is host-driven on Android: the Dart test prints `SHOOT:<name>`
  # to stdout when it has the right frame on screen, then sleeps briefly.
  # We tail flutter drive's combined output, and on each marker run
  # `adb exec-out screencap -p` to grab the pixels. This bypasses
  # `binding.takeScreenshot()`, which hangs indefinitely on Android
  # emulators under integration_test 4.x.
  set +e
  SCREENSHOT_OUT_DIR="$OUT_DIR" flutter drive \
    --driver=test_driver/integration_driver.dart \
    --target=integration_test/screenshot_test.dart \
    -d "$SERIAL" \
    --debug \
    --dart-define="API_BASE_URL=$API_URL" \
    --dart-define="TEST_ORG_CODE=$ORG_CODE" \
    --dart-define="TEST_USERNAME=$USERNAME" \
    --dart-define="TEST_PASSWORD=$PASSWORD" 2>&1 | while IFS= read -r line; do
    printf '%s\n' "$line"
    if [[ "$line" =~ SHOOT:([A-Za-z0-9_]+) ]]; then
      NAME="${BASH_REMATCH[1]}"
      if "$ADB" -s "$SERIAL" exec-out screencap -p > "$OUT_DIR/$NAME.png" 2>/dev/null \
         && [[ -s "$OUT_DIR/$NAME.png" ]]; then
        printf '    📸 captured → %s/%s.png\n' "$OUT_DIR" "$NAME"
      else
        printf '    ✗ screencap failed for %s\n' "$NAME" >&2
        rm -f "$OUT_DIR/$NAME.png"
      fi
    fi
  done
  set -e

  # Stop the emulator before the next class boots so adb device discovery
  # stays unambiguous.
  "$ADB" -s "$SERIAL" emu kill 2>/dev/null || true
  wait "$EMULATOR_PID" 2>/dev/null || true

  echo "──▶ $CLASS done"
done

echo
echo "All screenshots written under store_metadata/android/images/."
for d in phone-screenshots tablet-screenshots; do
  dir="$APP_ROOT/store_metadata/android/images/$d"
  if [[ -d "$dir" ]]; then
    count=$(find "$dir" -name '*.png' | wc -l | tr -d ' ')
    echo "  $d/  →  $count PNG(s)"
  fi
done
