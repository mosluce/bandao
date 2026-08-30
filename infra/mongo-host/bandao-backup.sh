#!/usr/bin/env bash
# Daily Mongo dump → age-encrypt → upload to S3.
# Reads config from /etc/bandao-backup.env (mode 0600, root-owned).
# Required keys:
#   MONGO_URI       full connection string for source DB (use a dump-scoped user)
#   MONGO_DB        database name to dump (e.g. "bandao")
#   AGE_RECIPIENT   age public key (e.g. "age1...")
#   S3_BUCKET       target bucket name
#   S3_REGION       AWS region of the bucket
#   S3_ACCESS_KEY_ID
#   S3_SECRET_ACCESS_KEY
# Optional:
#   S3_PREFIX          prefix inside the bucket (default "")
#   MIN_ARCHIVE_BYTES  reject a dump smaller than this (default 100000)
#   ASSERT_COLLECTION  collection to count before dumping (needs mongosh)
#   ASSERT_MIN_COUNT   minimum documents that collection must hold (default 1)
#   STAGING_DIR        where the archive is staged (default: mktemp under $TMPDIR)
#
# Schedule via the bundled systemd timer (bandao-backup.timer) at a
# low-traffic local hour. Logs go to syslog.
#
# The archive is staged to a local file and checked before it is uploaded,
# rather than streamed straight into `aws s3 cp -`. A stream cannot be
# inspected before it lands, which is how this pipeline spent weeks
# uploading dumps of the wrong database without anything going red: every
# command in it succeeded, and a mongodump of a near-empty database is a
# perfectly valid archive. Staging costs a few MB of scratch disk and buys
# the two guards below.
#
# Exit codes: 64 missing config, 65 pre-dump assertion failed,
# 66 archive smaller than MIN_ARCHIVE_BYTES.

set -Eeuo pipefail

ENV_FILE=${BANDAO_BACKUP_ENV:-/etc/bandao-backup.env}
if [[ ! -r "$ENV_FILE" ]]; then
  echo "missing config: $ENV_FILE" >&2
  exit 64
fi
# shellcheck disable=SC1090
set -a; . "$ENV_FILE"; set +a

: "${MONGO_URI:?missing MONGO_URI}"
: "${MONGO_DB:?missing MONGO_DB}"
: "${AGE_RECIPIENT:?missing AGE_RECIPIENT}"
: "${S3_BUCKET:?missing S3_BUCKET}"
: "${S3_REGION:?missing S3_REGION}"
: "${S3_ACCESS_KEY_ID:?missing S3_ACCESS_KEY_ID}"
: "${S3_SECRET_ACCESS_KEY:?missing S3_SECRET_ACCESS_KEY}"

min_bytes="${MIN_ARCHIVE_BYTES:-100000}"
assert_collection="${ASSERT_COLLECTION:-}"
assert_min_count="${ASSERT_MIN_COUNT:-1}"

prefix="${S3_PREFIX:-}"
prefix="${prefix%/}"
[[ -n "$prefix" ]] && prefix="${prefix}/"

stamp=$(date -u +%Y-%m-%dT%H-%M-%SZ)
key="${prefix}daily/${stamp}.archive.gz.age"

export AWS_ACCESS_KEY_ID="$S3_ACCESS_KEY_ID"
export AWS_SECRET_ACCESS_KEY="$S3_SECRET_ACCESS_KEY"
export AWS_DEFAULT_REGION="$S3_REGION"

fail() {
  local code=$1; shift
  logger -t bandao-backup -p user.err "FAILED: $*"
  echo "bandao-backup: $*" >&2
  exit "$code"
}

# Pre-dump sanity check: does the source database actually hold the data we
# think it does? This is the guard that would have caught the Zeabur
# migration leaving MONGO_URI pointed at the drained self-hosted mongod.
# Skipped when ASSERT_COLLECTION is unset, so the check stays opt-in for
# anyone running this against a database with a different shape.
if [[ -n "$assert_collection" ]]; then
  if ! command -v mongosh >/dev/null 2>&1; then
    fail 65 "ASSERT_COLLECTION is set but mongosh is not installed"
  fi
  count=$(mongosh "$MONGO_URI" --quiet --eval \
    "db.getSiblingDB('${MONGO_DB}').getCollection('${assert_collection}').countDocuments({})" \
    2>/dev/null | tr -dc '0-9') || true
  if [[ -z "$count" ]]; then
    fail 65 "could not count ${MONGO_DB}.${assert_collection} — is MONGO_URI reachable?"
  fi
  if (( count < assert_min_count )); then
    fail 65 "${MONGO_DB}.${assert_collection} holds ${count} documents, expected >= ${assert_min_count} — refusing to overwrite good backups with a dump of the wrong database"
  fi
  logger -t bandao-backup "pre-dump check ok: ${MONGO_DB}.${assert_collection}=${count}"
fi

staging_dir=$(mktemp -d "${STAGING_DIR:-${TMPDIR:-/tmp}}/bandao-backup.XXXXXX")
trap 'rm -rf "$staging_dir"' EXIT
archive="$staging_dir/${stamp}.archive.gz.age"

logger -t bandao-backup "starting dump db=$MONGO_DB → s3://$S3_BUCKET/$key"

mongodump --uri="$MONGO_URI" --db="$MONGO_DB" --gzip --archive \
  | age -r "$AGE_RECIPIENT" \
  > "$archive"

size=$(wc -c < "$archive" | tr -d ' ')

# A dump of the wrong (or emptied) database is still a well-formed archive,
# so size is the only signal left once every command has exited 0. The floor
# is deliberately blunt: it cannot tell a truncated dump from a real one, it
# only refuses to let an obviously-too-small archive rotate a good backup out
# of the 30-day window.
if (( size < min_bytes )); then
  fail 66 "archive is ${size} bytes, below MIN_ARCHIVE_BYTES=${min_bytes} — not uploading"
fi

aws s3 cp --no-progress "$archive" "s3://$S3_BUCKET/$key"

logger -t bandao-backup "uploaded s3://$S3_BUCKET/$key (${size} bytes)"
