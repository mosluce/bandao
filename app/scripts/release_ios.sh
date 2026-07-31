#!/usr/bin/env bash
#
# Cut a new iOS release end-to-end: bump pubspec build number, build the
# .ipa with the production API URL baked in, upload to App Store Connect.
#
# Combines what `upload_ios.sh` does (operator-facing API key handling)
# with the missing pieces from a previous failed flow:
#   - bumping +<build> in pubspec.yaml so Apple accepts the upload
#     (the same counter is Android's versionCode; it never resets)
#   - passing --dart-define=API_BASE_URL so the .ipa actually points at
#     prod (without it, Env.compileTimeDefault falls back to localhost
#     and TestFlight users can't log in)
#   - passing --dart-define=PRIVACY_URL so the location-consent dialog's
#     privacy link matches the URL declared to App Review (without it the
#     link resolves to localhost:3000 while the store listing advertises
#     the real one)
#   - verifying, after the build, that both URLs are actually inside the
#     .ipa before uploading it. Android shipped 0.4.3+13 to production
#     with a loopback API URL baked in because nothing checked; this
#     script asserts the artifact rather than trusting its own flags.
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
# Bump marketing version too (e.g. 0.3.0 → 0.4.0). The build number keeps
# climbing — it never resets, because it doubles as Android's versionCode:
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
# Read the privacy URL from the file that also feeds the App Store listing,
# so the in-app link and the URL declared to review cannot drift apart.
PRIVACY_URL_FILE="$(cd "$(dirname "$0")/.." && pwd)/store_metadata/ios/privacy_url.txt"
PRIVACY_URL=""
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
  --name X.Y.Z         Set marketing version. The build number still just
                       increments — it never resets (it is Android's
                       versionCode, which must climb globally).
  --no-bump            Don't change pubspec.yaml at all (use existing
                       version+build). Useful when retrying a rejected
                       upload before Apple processed it.
  --no-upload          Build only, skip upload. Defaults to false.
  --api URL            API base URL to bake in. Default: env var
                       BANDAO_API_URL or https://bandao-api.ccmos.tw.
  --privacy URL        Privacy policy URL to bake in. Default: contents of
                       store_metadata/ios/privacy_url.txt.
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
    --privacy)     PRIVACY_URL="$2"; shift 2 ;;
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

if [[ -z "$PRIVACY_URL" ]]; then
  if [[ ! -f "$PRIVACY_URL_FILE" ]]; then
    echo "Missing $PRIVACY_URL_FILE — cannot resolve the privacy policy URL." >&2
    echo "Pass --privacy URL explicitly, or restore the store metadata file." >&2
    exit 2
  fi
  PRIVACY_URL="$(tr -d '[:space:]' < "$PRIVACY_URL_FILE")"
fi

if [[ -z "$PRIVACY_URL" ]]; then
  echo "Privacy policy URL resolved to an empty string." >&2
  exit 2
fi

# ── Artifact verification ───────────────────────────────────────────────
#
# Keep in sync with the identical function in release_android.sh. These
# scripts stay standalone on purpose — an operator can copy one to a
# release machine without dragging a lib/ directory along — so the check
# is duplicated rather than sourced.
#
# Assert that each URL is present in every Dart snapshot inside the archive.
#
# Two things about this check are easy to get wrong:
#
# 1. It asserts PRESENCE only, never absence. "Fail if the binary mentions
#    localhost" is the obvious check and it does not work: whether a dev
#    loopback literal survives into a release artifact is not predictable.
#    A correctly built .ipa contains `http://localhost:9090` and the
#    production URL side by side, while on Android the privacy loopback is
#    shaken out of a correct bundle and the API one is not. An absence
#    check would red-flag good releases on one URL and never fire on the
#    other.
#
# 2. It extracts to a temp file instead of `unzip -p ... | grep -q`. With
#    `set -o pipefail`, a `grep -q` that matches early can leave the pipe
#    writer with SIGPIPE and surface as a failed pipeline on a passing
#    check. macOS's Info-ZIP happens to tolerate it; that is not a property
#    to depend on in the one script whose job is to be trustworthy.
#
# A member path that cannot be found is a failure, not a skip: if the
# archive layout changes under us, the release must stop rather than
# silently assert nothing.
verify_urls_in_artifact() {
  local archive="$1" member_glob="$2"
  shift 2
  local urls=("$@")

  local members
  members="$(unzip -Z1 "$archive" "$member_glob" 2>/dev/null || true)"
  if [[ -z "$members" ]]; then
    echo >&2
    echo "FATAL: no archive member matched '$member_glob' in" >&2
    echo "       $archive" >&2
    echo "       The artifact layout changed — this check verified nothing," >&2
    echo "       so the release is stopping rather than assuming it passed." >&2
    return 1
  fi

  # "Could not verify" and "verified and failed" both stop the release, but
  # they are different facts and must not share a message. An earlier cut of
  # this function let a failed mktemp fall through to the grep, which then
  # reported a URL as missing from a bundle that plainly contained it —
  # sending the operator to debug a build problem that did not exist.
  local tmp
  if ! tmp="$(mktemp -d)"; then
    echo >&2
    echo "FATAL: could not create a temp directory to unpack the artifact." >&2
    echo "       Verification did not run, so it did not pass." >&2
    return 1
  fi

  local member url rc=0
  while IFS= read -r member; do
    [[ -z "$member" ]] && continue
    if ! unzip -p "$archive" "$member" > "$tmp/snapshot" 2>/dev/null; then
      echo >&2
      echo "FATAL: could not extract $member from" >&2
      echo "       $archive" >&2
      echo "       Verification did not run, so it did not pass." >&2
      rc=1
      break
    fi
    for url in "${urls[@]}"; do
      if ! grep -a -q -F "$url" "$tmp/snapshot"; then
        echo >&2
        echo "FATAL: $url" >&2
        echo "       is not present in $member" >&2
        echo "       The --dart-define did not reach the build. This archive" >&2
        echo "       would ship pointing at a development loopback address." >&2
        rc=1
        break 2
      fi
    done
    echo "    ✓ $member"
  done <<< "$members"

  rm -rf "$tmp"
  return $rc
}

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
  # The build number keeps climbing across a marketing bump — it does NOT
  # reset. It is Android's `versionCode` too, and Play requires that to
  # increase globally and forever; a reset makes every subsequent upload
  # rejected outright. Apple only needs it to increase within one marketing
  # version, so the stricter rule wins. See DEPLOY.md "Version numbering".
  if [[ $BUMP_BUILD -eq 1 ]]; then
    TARGET_BUILD=$((CURRENT_BUILD + 1))
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
echo "    Privacy URL:  $PRIVACY_URL"
flutter build ipa --release \
  --dart-define="API_BASE_URL=$API_URL" \
  --dart-define="PRIVACY_URL=$PRIVACY_URL"

IPA_FILE="$(find "$APP_ROOT/build/ios/ipa" -maxdepth 1 -name '*.ipa' -type f \
            | head -1)"
if [[ -z "$IPA_FILE" || ! -f "$IPA_FILE" ]]; then
  echo "Build said success but no .ipa under build/ios/ipa/ — check above output." >&2
  exit 1
fi

echo "──▶ Built $IPA_FILE"

# ── Verify the artifact, not the invocation ─────────────────────────────
echo "──▶ Verifying the archive carries the production URLs"
verify_urls_in_artifact "$IPA_FILE" 'Payload/*.app/Frameworks/App.framework/App' \
  "$API_URL" "$PRIVACY_URL"
echo "    Archive verified."

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
