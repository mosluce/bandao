#!/usr/bin/env bash
#
# Cut a new iOS release end-to-end: bump pubspec build number, build the
# .ipa with the production API URL baked in, upload to App Store Connect.
#
# Combines what `upload_ios.sh` does (operator-facing API key handling)
# with the missing pieces from a previous failed flow:
#   - bumping +<build> in pubspec.yaml so Apple accepts the upload
#   - passing --dart-define=API_BASE_URL so the .ipa actually points at
#     prod (without it, Env.compileTimeDefault falls back to localhost
#     and TestFlight users can't log in)
#
# One-time prerequisites (see DEPLOY.md):
#   - APP_STORE_CONNECT_API_KEY_ID + _ISSUER_ID env vars set
#   - .p8 at ~/.appstoreconnect/private_keys/AuthKey_<KEY_ID>.p8
#   - Apple distribution certificate in keychain (Xcode auto-fetches once
#     DEVELOPMENT_TEAM in project.pbxproj is correct)
#
# Common usage (defaults: bump +build, target prod, upload):
#   ./scripts/release_ios.sh
#
# Bump marketing version too (e.g. 0.3.0 → 0.4.0, build resets to 1):
#   ./scripts/release_ios.sh --name 0.4.0
#
# Re-cut same version+build (e.g. previous upload was rejected by Apple
# before processing — you need to re-upload identical version):
#   ./scripts/release_ios.sh --no-bump
#
# Build only, don't upload (useful for local smoke / TestFlight testing
# via Xcode):
#   ./scripts/release_ios.sh --no-upload

set -euo pipefail

# ── Defaults ────────────────────────────────────────────────────────────
KEY_ID="${APP_STORE_CONNECT_API_KEY_ID:-}"
ISSUER_ID="${APP_STORE_CONNECT_API_ISSUER_ID:-}"
API_URL="${BANDAO_API_URL:-https://bandao-api.ccmos.tw}"
BUMP_BUILD=1   # bump +N → +(N+1) by default
NEW_NAME=""    # optional: override marketing version (X.Y.Z)
DO_UPLOAD=1

usage() {
  cat <<'USAGE'
Usage: release_ios.sh [options]

Bumps pubspec.yaml's build number, runs `flutter build ipa --release` with
the prod API URL baked in, then uploads to App Store Connect via
`xcrun altool`.

Options:
  --name X.Y.Z         Set marketing version (also resets build to 1).
  --no-bump            Don't change pubspec.yaml at all (use existing
                       version+build). Useful when retrying a rejected
                       upload before Apple processed it.
  --no-upload          Build only, skip upload. Defaults to false.
  --api URL            API base URL to bake in. Default: env var
                       BANDAO_API_URL or https://bandao-api.ccmos.tw.
  --key-id ID          App Store Connect API Key ID.
                       (Default: env var APP_STORE_CONNECT_API_KEY_ID.)
  --issuer-id UUID     App Store Connect API Issuer ID.
                       (Default: env var APP_STORE_CONNECT_API_ISSUER_ID.)
  -h, --help           Print this help.
USAGE
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --name)        NEW_NAME="$2"; shift 2 ;;
    --no-bump)     BUMP_BUILD=0; shift ;;
    --no-upload)   DO_UPLOAD=0; shift ;;
    --api)         API_URL="$2"; shift 2 ;;
    --key-id)      KEY_ID="$2"; shift 2 ;;
    --issuer-id)   ISSUER_ID="$2"; shift 2 ;;
    -h|--help)     usage; exit 0 ;;
    *)             echo "Unknown option: $1" >&2; usage; exit 2 ;;
  esac
done

if [[ $DO_UPLOAD -eq 1 ]]; then
  if [[ -z "$KEY_ID" || -z "$ISSUER_ID" ]]; then
    echo "Missing App Store Connect API credentials." >&2
    echo "Either set APP_STORE_CONNECT_API_KEY_ID + _ISSUER_ID env vars," >&2
    echo "or pass --key-id / --issuer-id, or pass --no-upload." >&2
    exit 2
  fi
  if [[ ! -f "$HOME/.appstoreconnect/private_keys/AuthKey_$KEY_ID.p8" ]]; then
    echo "Missing .p8 at ~/.appstoreconnect/private_keys/AuthKey_$KEY_ID.p8" >&2
    echo "See DEPLOY.md → 'App Store Connect API key (one-time operator setup)'" >&2
    exit 2
  fi
fi

APP_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$APP_ROOT"

# ── Bump pubspec.yaml ───────────────────────────────────────────────────
CURRENT_LINE="$(grep -E '^version: ' pubspec.yaml | head -1)"
CURRENT_VERSION="$(echo "$CURRENT_LINE" | sed -E 's/^version: *//')"
CURRENT_NAME="${CURRENT_VERSION%+*}"
CURRENT_BUILD="${CURRENT_VERSION#*+}"

if [[ "$CURRENT_VERSION" == "$CURRENT_NAME" ]]; then
  # No `+build` segment — treat current build as 0.
  CURRENT_BUILD=0
fi

if [[ -n "$NEW_NAME" ]]; then
  TARGET_NAME="$NEW_NAME"
  # When marketing version changes, reset build to 1 unless --no-bump.
  if [[ $BUMP_BUILD -eq 1 ]]; then
    TARGET_BUILD=1
  else
    TARGET_BUILD="$CURRENT_BUILD"
  fi
else
  TARGET_NAME="$CURRENT_NAME"
  if [[ $BUMP_BUILD -eq 1 ]]; then
    TARGET_BUILD=$((CURRENT_BUILD + 1))
  else
    TARGET_BUILD="$CURRENT_BUILD"
  fi
fi

TARGET_VERSION="$TARGET_NAME+$TARGET_BUILD"

if [[ "$CURRENT_VERSION" != "$TARGET_VERSION" ]]; then
  echo "──▶ pubspec.yaml: $CURRENT_VERSION  →  $TARGET_VERSION"
  # macOS sed needs '' for in-place; -E for ERE.
  sed -i '' -E "s/^version: .+\$/version: $TARGET_VERSION/" pubspec.yaml
else
  echo "──▶ pubspec.yaml unchanged at $CURRENT_VERSION (--no-bump in effect)"
fi

# ── flutter pub get + build ─────────────────────────────────────────────
echo "──▶ flutter pub get"
flutter pub get >/dev/null

echo "──▶ flutter build ipa --release"
echo "    API base URL: $API_URL"
flutter build ipa --release \
  --dart-define="API_BASE_URL=$API_URL"

IPA_FILE="$(find "$APP_ROOT/build/ios/ipa" -maxdepth 1 -name '*.ipa' -type f \
            | head -1)"
if [[ -z "$IPA_FILE" || ! -f "$IPA_FILE" ]]; then
  echo "Build said success but no .ipa under build/ios/ipa/ — check above output." >&2
  exit 1
fi

echo "──▶ Built $IPA_FILE"

# ── Upload ──────────────────────────────────────────────────────────────
if [[ $DO_UPLOAD -eq 0 ]]; then
  echo
  echo "Skipping upload (--no-upload). To upload manually:"
  echo "  ./scripts/upload_ios.sh"
  echo
  exit 0
fi

echo "──▶ Uploading $IPA_FILE to App Store Connect"
xcrun altool --upload-app \
  --type ios \
  --file "$IPA_FILE" \
  --apiKey "$KEY_ID" \
  --apiIssuer "$ISSUER_ID"

# ── Reminder ────────────────────────────────────────────────────────────
echo
echo "──▶ Upload accepted. Apple will email when processing finishes."
echo "    Check App Store Connect → My Apps → 班到 → TestFlight in 10-30 min."
echo
echo "Don't forget to commit the version bump if pubspec changed:"
echo "  git add app/pubspec.yaml"
echo "  git commit -m 'chore(app): bump iOS release to $TARGET_VERSION'"
echo "  git tag app-v$TARGET_NAME"
echo "  git push --follow-tags"
