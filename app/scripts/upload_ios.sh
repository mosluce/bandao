#!/usr/bin/env bash
#
# Upload the latest .ipa to App Store Connect via xcrun altool.
#
# Equivalent to `fastlane pilot upload` but without the fastlane stack
# — same App Store Connect API underneath.
#
# One-time operator setup (off-repo):
#   1. Apple Developer Portal → Users and Access → Integrations →
#      App Store Connect API → Generate API Key (role: App Manager).
#   2. Download the AuthKey_<KEY_ID>.p8 file (Apple only lets you
#      download it once).
#   3. Move it to ~/.appstoreconnect/private_keys/AuthKey_<KEY_ID>.p8
#      (altool's auto-discovery path).
#   4. Save .p8 + Key ID + Issuer ID to the password manager as a
#      single item (binary attachment + two custom fields).
#
# Each invocation:
#   export APP_STORE_CONNECT_API_KEY_ID=ABC123XYZ4
#   export APP_STORE_CONNECT_API_ISSUER_ID=12345678-1234-1234-1234-123456789012
#   ./scripts/upload_ios.sh
#
# Or pass via flags:
#   ./scripts/upload_ios.sh --key-id ABC... --issuer-id 12345...

set -euo pipefail

KEY_ID="${APP_STORE_CONNECT_API_KEY_ID:-}"
ISSUER_ID="${APP_STORE_CONNECT_API_ISSUER_ID:-}"
BUILD_FIRST=0

usage() {
  cat <<'USAGE'
Usage: upload_ios.sh [options]

Required (also accept env vars: APP_STORE_CONNECT_API_KEY_ID / _ISSUER_ID):
  --key-id     ID    App Store Connect API Key ID (~10-char alphanumeric).
  --issuer-id  UUID  App Store Connect API Issuer ID (UUID).

Optional:
  --build            Run `flutter build ipa --release` first.
  -h, --help         Print this help.

The .p8 private key must live at:
  ~/.appstoreconnect/private_keys/AuthKey_<KEY_ID>.p8
This is altool's default auto-discovery path; no flag needed.

The .ipa to upload is whatever's in app/build/ios/ipa/. Run with --build
or run `flutter build ipa --release` first.
USAGE
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --key-id)     KEY_ID="$2"; shift 2 ;;
    --issuer-id)  ISSUER_ID="$2"; shift 2 ;;
    --build)      BUILD_FIRST=1; shift ;;
    -h|--help)    usage; exit 0 ;;
    *) echo "Unknown option: $1" >&2; usage; exit 2 ;;
  esac
done

if [[ -z "$KEY_ID" || -z "$ISSUER_ID" ]]; then
  echo "Missing API key info." >&2
  usage
  exit 2
fi

# Verify the .p8 file is present where altool expects it.
KEY_FILE="$HOME/.appstoreconnect/private_keys/AuthKey_$KEY_ID.p8"
if [[ ! -f "$KEY_FILE" ]]; then
  cat >&2 <<EOF
.p8 key file not found at: $KEY_FILE

altool auto-discovers private keys at one of:
  ~/.appstoreconnect/private_keys/AuthKey_<KEY_ID>.p8
  ~/private_keys/AuthKey_<KEY_ID>.p8

Move the .p8 you downloaded from App Store Connect there, e.g.:
  mkdir -p ~/.appstoreconnect/private_keys
  mv ~/Downloads/AuthKey_$KEY_ID.p8 ~/.appstoreconnect/private_keys/

If you've lost the .p8, regenerate via Apple Developer Portal →
Users and Access → Integrations → App Store Connect API.
EOF
  exit 2
fi

APP_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$APP_ROOT"

# Optional: rebuild the .ipa first. Skipping this is the default because
# `flutter build ipa --release` takes ~1 min and you may want to upload
# a build that's already validated locally.
if [[ $BUILD_FIRST -eq 1 ]]; then
  echo "──▶ flutter build ipa --release"
  flutter build ipa --release
fi

# Locate the .ipa. Flutter writes a single one per build at
# build/ios/ipa/<AppName>.ipa.
IPA_DIR="$APP_ROOT/build/ios/ipa"
if [[ ! -d "$IPA_DIR" ]]; then
  echo "No build/ios/ipa/ directory. Run with --build or" >&2
  echo "  cd app && flutter build ipa --release" >&2
  exit 2
fi

IPA_FILE="$(find "$IPA_DIR" -maxdepth 1 -name '*.ipa' -type f | head -1)"
if [[ -z "$IPA_FILE" ]]; then
  echo "No .ipa found under $IPA_DIR." >&2
  echo "Run with --build or " >&2
  echo "  cd app && flutter build ipa --release" >&2
  exit 2
fi

echo "──▶ Uploading $IPA_FILE"
echo "    Key ID:    $KEY_ID"
echo "    Issuer ID: $ISSUER_ID"
echo

# `altool --upload-app` is Apple's officially supported iOS upload path.
# It uses the App Store Connect API under the hood (same as fastlane
# pilot). Successful upload returns 0 and prints a delivery confirmation
# message; the build then takes 10-30 min to appear in TestFlight as
# Apple processes it.
xcrun altool --upload-app \
  --type ios \
  --file "$IPA_FILE" \
  --apiKey "$KEY_ID" \
  --apiIssuer "$ISSUER_ID"

echo
echo "──▶ Upload accepted. Apple will email when processing finishes."
echo "    Check App Store Connect → My Apps → 班到 → TestFlight in 10-30 min."
