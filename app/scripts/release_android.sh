#!/usr/bin/env bash
#
# Cut a new Android release: build the .aab with the production URLs baked
# in, verify the artifact actually carries them, hand off to upload.
#
# The Android counterpart of release_ios.sh, written after 0.4.3+13 shipped
# to Play production with `http://10.0.2.2:9090` as its API base URL. That
# happened because DEPLOY.md's Android procedure was a bare
# `flutter build appbundle --release`: with no --dart-define,
# Env.compileTimeDefault() falls back to the emulator's alias for the build
# host's loopback, which means nothing on a device — and the app ships no
# cleartext exception, so the plain-HTTP request is blocked besides. The
# bundle was well-formed and correctly signed. It simply could not reach
# the backend, and nothing in the pipeline noticed.
#
# So this script bakes BOTH dart-defines and then re-opens the bundle it
# just produced to prove they took effect. See `verify_urls_in_artifact`.
#
# Unlike release_ios.sh, this does NOT touch pubspec.yaml. The build number
# is a single counter shared with iOS, and DEPLOY.md requires both stores be
# cut from the same number so one number identifies one binary pair. A
# second bumping script would desync them the first time both were cut.
# Bump deliberately — via release_ios.sh, or by hand — before running this.
#
# One-time prerequisites (see DEPLOY.md):
#   - android/key.properties populated (upload keystore, off-repo)
#   - PLAY_JSON_KEY pointing at the Play service-account JSON, for upload
#
# Common usage (build at current pubspec version, verify, upload to
# the `internal` track):
#   export PLAY_JSON_KEY=~/.bandao/keystores/<project>-<id>.json
#   ./scripts/release_android.sh
#
# Build and verify only, don't upload:
#   ./scripts/release_android.sh --no-upload

set -euo pipefail

APP_ROOT="$(cd "$(dirname "$0")/.." && pwd)"

# ── Defaults ────────────────────────────────────────────────────────────
API_URL="${BANDAO_API_URL:-https://bandao-api.ccmos.tw}"
# Read the privacy URL from the file that also feeds the Play listing, so
# the in-app link and the URL declared to review cannot drift apart. Both
# shipped binaries once linked a loopback address while both store listings
# declared the real one; a constant restated in each script is how that
# comes back.
PRIVACY_URL_FILE="$APP_ROOT/store_metadata/android/privacy_policy_url.txt"
PRIVACY_URL=""
DO_UPLOAD=1

usage() {
  cat <<'USAGE'
Usage: release_android.sh [options]

Runs `flutter build appbundle --release` with the production API base URL
and privacy policy URL baked in, verifies the produced .aab actually
carries them, then hands off to upload_android.sh.

Does NOT modify pubspec.yaml — the build number is shared with iOS and is
bumped once, deliberately, elsewhere.

Options:
  --no-upload          Build and verify only, skip upload.
  --api URL            API base URL to bake in. Default: env var
                       BANDAO_API_URL or https://bandao-api.ccmos.tw.
  --privacy URL        Privacy policy URL to bake in. Default: contents of
                       store_metadata/android/privacy_policy_url.txt.
  -h, --help           Print this help.

Anything else is passed through to upload_android.sh, e.g.
  ./scripts/release_android.sh --json-key ~/.bandao/keystores/x.json
USAGE
}

UPLOAD_ARGS=()
while [[ $# -gt 0 ]]; do
  case "$1" in
    --no-upload)   DO_UPLOAD=0; shift ;;
    --api)         API_URL="$2"; shift 2 ;;
    --privacy)     PRIVACY_URL="$2"; shift 2 ;;
    -h|--help)     usage; exit 0 ;;
    *)             UPLOAD_ARGS+=("$1"); shift ;;
  esac
done

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
# Assert that each URL is present in every Dart snapshot inside the archive.
#
# Two things about this check are easy to get wrong:
#
# 1. It asserts PRESENCE only, never absence. "Fail if the binary mentions
#    10.0.2.2" is the obvious check and it does not work: whether a dev
#    loopback literal survives into a release artifact is not predictable.
#    Measured on two real bundles — in a correctly built one the privacy
#    loopback is gone (its define makes env.dart's fallback branch dead
#    code, which gets shaken out) while `http://10.0.2.2:9090` survives
#    anyway. An absence check would red-flag good releases on one URL and
#    never fire on the other.
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
        echo "       The --dart-define did not reach the build. This bundle" >&2
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

cd "$APP_ROOT"

VERSION="$(grep -E '^version: ' pubspec.yaml | head -1 | sed -E 's/^version: *//')"

echo "──▶ Building Android release at pubspec version $VERSION"
echo "    (pubspec is not modified by this script — the build number is"
echo "     shared with iOS and bumped deliberately elsewhere)"
echo "    API base URL:  $API_URL"
echo "    Privacy URL:   $PRIVACY_URL"

# ── Build ───────────────────────────────────────────────────────────────
echo "──▶ flutter pub get"
flutter pub get >/dev/null

echo "──▶ dart run build_runner build"
dart run build_runner build --delete-conflicting-outputs >/dev/null

echo "──▶ flutter build appbundle --release"
flutter build appbundle --release \
  --dart-define="API_BASE_URL=$API_URL" \
  --dart-define="PRIVACY_URL=$PRIVACY_URL"

AAB_FILE="$APP_ROOT/build/app/outputs/bundle/release/app-release.aab"
if [[ ! -f "$AAB_FILE" ]]; then
  echo "Build said success but no .aab at $AAB_FILE — check above output." >&2
  exit 1
fi

echo "──▶ Built $AAB_FILE"

# ── Verify the artifact, not the invocation ─────────────────────────────
echo "──▶ Verifying the bundle carries the production URLs"
verify_urls_in_artifact "$AAB_FILE" 'base/lib/*/libapp.so' \
  "$API_URL" "$PRIVACY_URL"
echo "    Bundle verified."

# ── Upload ──────────────────────────────────────────────────────────────
if [[ $DO_UPLOAD -eq 0 ]]; then
  echo
  echo "Skipping upload (--no-upload). To upload manually:"
  echo "  ./scripts/upload_android.sh"
  echo
  exit 0
fi

echo "──▶ Handing off to upload_android.sh"
# upload_android.sh defaults to the `internal` track and demands typed
# confirmation for anything wider. Promotion stays a judgement call; this
# script does not make it one command.
"$APP_ROOT/scripts/upload_android.sh" "${UPLOAD_ARGS[@]+"${UPLOAD_ARGS[@]}"}"
