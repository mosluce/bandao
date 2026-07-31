#!/usr/bin/env bash
#
# Upload the release .aab to Google Play via `fastlane supply`.
#
# The Android counterpart of upload_ios.sh. Same shape: the build is a
# separate step, this only uploads what is already on disk.
#
# Defaults to the `internal` track, deliberately. Promotion to closed or
# production is a judgement call about whether a build is fit to ship, and
# routing that through a script invites promoting one that is not.
#
# One-time operator setup (off-repo):
#   1. GCP Console → IAM → Service Accounts → create one (no project roles
#      needed; Play permissions are NOT granted through GCP IAM).
#   2. That service account → Keys → Add key → JSON. Downloadable once.
#   3. Store it outside the repo, e.g.
#      ~/.bandao/keystores/<project>-<id>.json, and save it to the password
#      manager alongside the Play upload keystore.
#   4. GCP Console → APIs & Services → Library → enable
#      "Google Play Android Developer API" in that same project. Separate
#      from step 5; missing it yields 403 SERVICE_DISABLED even when the
#      credentials are correct.
#   5. Play Console → Users and permissions (top-level nav, NOT inside
#      Developer account settings — that page no longer carries API access)
#      → Invite new user → paste the service account email → grant app
#      permissions for 班到 only, not account-level. A service account has
#      no inbox, so the invitation takes effect immediately with no
#      acceptance step.
#
# Verify the whole chain before trusting it:
#   fastlane run validate_play_store_json_key json_key:<path>
#   → "Successfully established connection to Google Play Store."
#
# Each invocation:
#   export PLAY_JSON_KEY=~/.bandao/keystores/<project>-<id>.json
#   ./scripts/upload_android.sh
#
# Or pass via flag:
#   ./scripts/upload_android.sh --json-key ~/.bandao/keystores/...json

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
APP_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

JSON_KEY="${PLAY_JSON_KEY:-}"
TRACK="internal"
AAB=""
DRY_RUN=0
SKIP_NOTES=0

usage() {
  cat <<'USAGE'
Usage: upload_android.sh [options]

Required (also accepts env var PLAY_JSON_KEY):
  --json-key PATH    Service account JSON for the Play Developer API.

Optional:
  --track NAME       internal (default) | alpha | beta | production.
                     Anything other than internal asks for confirmation.
  --aab PATH         Bundle to upload. Defaults to the release output.
  --skip-notes       Upload without release notes. By default the upload
                     aborts if store_metadata/android/changelog/<code>.txt
                     is missing, so a release never reaches testers with an
                     empty "what's new".
  --dry-run          Validate everything and print what would be uploaded.
  -h, --help         This message.
USAGE
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --json-key)   JSON_KEY="${2:-}"; shift 2 ;;
    --track)      TRACK="${2:-}"; shift 2 ;;
    --aab)        AAB="${2:-}"; shift 2 ;;
    --skip-notes) SKIP_NOTES=1; shift ;;
    --dry-run)    DRY_RUN=1; shift ;;
    -h|--help)    usage; exit 0 ;;
    *) echo "Unknown option: $1" >&2; usage; exit 2 ;;
  esac
done

if [[ -z "$JSON_KEY" ]]; then
  echo "ERROR: no service account JSON. Pass --json-key or set PLAY_JSON_KEY." >&2
  exit 2
fi
JSON_KEY="${JSON_KEY/#\~/$HOME}"
if [[ ! -f "$JSON_KEY" ]]; then
  echo "ERROR: json key not found: $JSON_KEY" >&2
  exit 2
fi

# A credential that lands inside the repo is one `git add -A` away from
# being published, and this one can publish to Play production.
case "$(cd "$(dirname "$JSON_KEY")" && pwd)" in
  "$APP_DIR"|"$APP_DIR"/*|"$(cd "$APP_DIR/.." && pwd)"/*)
    echo "ERROR: the service account JSON is inside the repository:" >&2
    echo "  $JSON_KEY" >&2
    echo "Move it out (e.g. ~/.bandao/keystores/) and retry." >&2
    exit 2 ;;
esac

if [[ -z "$AAB" ]]; then
  AAB="$APP_DIR/build/app/outputs/bundle/release/app-release.aab"
fi
if [[ ! -f "$AAB" ]]; then
  echo "ERROR: bundle not found: $AAB" >&2
  echo "Build it first: flutter build appbundle --release" >&2
  exit 2
fi

PKG="$(grep -oE 'applicationId = "[^"]+"' "$APP_DIR/android/app/build.gradle.kts" \
  | head -1 | sed -E 's/.*"([^"]+)".*/\1/')"
if [[ -z "$PKG" ]]; then
  echo "ERROR: could not read applicationId from build.gradle.kts" >&2
  exit 2
fi

# Read versionCode out of the bundle rather than from pubspec.yaml. The two
# can disagree — pubspec may have moved on since the bundle was built — and
# the number that matters for Play is the one actually inside the artifact.
VERSION_CODE="$(python3 - "$AAB" <<'PY'
import re, sys, zipfile
raw = zipfile.ZipFile(sys.argv[1]).read("base/manifest/AndroidManifest.xml")
i = raw.find(b"versionCode")
m = re.match(rb"versionCode\x1a(.)([0-9]+)", raw[i:])
print(m.group(2).decode() if m else "")
PY
)"
if [[ -z "$VERSION_CODE" ]]; then
  echo "ERROR: could not read versionCode from $AAB" >&2
  exit 2
fi

NOTES_FILE="$APP_DIR/store_metadata/android/changelog/${VERSION_CODE}.txt"
SUPPLY_METADATA=()
if [[ -f "$NOTES_FILE" ]]; then
  # supply expects metadata/<locale>/changelogs/<versionCode>.txt; build that
  # layout in a temp dir so the repo keeps its flat, human-editable one.
  STAGE="$(mktemp -d)"
  trap 'rm -rf "$STAGE"' EXIT
  mkdir -p "$STAGE/zh-TW/changelogs"
  cp "$NOTES_FILE" "$STAGE/zh-TW/changelogs/${VERSION_CODE}.txt"
  SUPPLY_METADATA=(metadata_path:"$STAGE")
elif [[ $SKIP_NOTES -eq 0 ]]; then
  echo "ERROR: no release notes for versionCode $VERSION_CODE." >&2
  echo "  expected: $NOTES_FILE" >&2
  echo "Write them, or pass --skip-notes to upload without." >&2
  exit 2
else
  echo "WARNING: uploading versionCode $VERSION_CODE with no release notes."
fi

echo "package      : $PKG"
echo "bundle       : $AAB"
echo "versionCode  : $VERSION_CODE"
echo "track        : $TRACK"
echo "release notes: ${NOTES_FILE/#$APP_DIR\//}"
[[ ${#SUPPLY_METADATA[@]} -eq 0 ]] && echo "               (none — --skip-notes)"

if [[ "$TRACK" != "internal" ]]; then
  echo
  echo "Track '$TRACK' reaches real users. Type the track name to confirm:"
  read -r CONFIRM
  if [[ "$CONFIRM" != "$TRACK" ]]; then
    echo "Aborted." >&2
    exit 1
  fi
fi

if [[ $DRY_RUN -eq 1 ]]; then
  echo
  echo "--dry-run: validating credentials only, nothing will be uploaded."
  FASTLANE_SKIP_UPDATE_CHECK=1 fastlane run validate_play_store_json_key \
    json_key:"$JSON_KEY"
  exit 0
fi

FASTLANE_SKIP_UPDATE_CHECK=1 fastlane run supply \
  json_key:"$JSON_KEY" \
  package_name:"$PKG" \
  aab:"$AAB" \
  track:"$TRACK" \
  release_status:"completed" \
  skip_upload_apk:true \
  skip_upload_metadata:true \
  skip_upload_images:true \
  skip_upload_screenshots:true \
  "${SUPPLY_METADATA[@]}"

echo
echo "Uploaded versionCode $VERSION_CODE to the '$TRACK' track."
echo "Promote internal → closed → production from the Play Console after smoke."
