#!/usr/bin/env bash
# Monthly restore drill: pull latest daily/*.age, decrypt, restore into a
# scratch DB, assert non-trivial document counts, drop the scratch DB.
#
# The operator's age private key is supplied via:
#   - SOPS / a sealed file unlocked at drill time, or
#   - stdin (--key -), or
#   - $AGE_IDENTITY_FILE pointing at a temporary mount.
# This script accepts the key path as $AGE_IDENTITY_FILE; it must NOT be
# stored permanently on the Mongo host.
#
# Required env (typically sourced from the same /etc/bandao-backup.env
# plus AGE_IDENTITY_FILE):
#   MONGO_URI, S3_BUCKET, S3_REGION, S3_ACCESS_KEY_ID, S3_SECRET_ACCESS_KEY
#   AGE_IDENTITY_FILE  path to private key file (deleted after run)
#   ASSERT_COLLECTION  collection name expected to be non-empty (e.g. "checkin_events")
#   ASSERT_MIN_COUNT   minimum acceptable document count (e.g. 1)

set -Eeuo pipefail

ENV_FILE=${BANDAO_BACKUP_ENV:-/etc/bandao-backup.env}
if [[ -r "$ENV_FILE" ]]; then
  # shellcheck disable=SC1090
  set -a; . "$ENV_FILE"; set +a
fi

: "${MONGO_URI:?missing MONGO_URI}"
: "${S3_BUCKET:?missing S3_BUCKET}"
: "${S3_REGION:?missing S3_REGION}"
: "${S3_ACCESS_KEY_ID:?missing S3_ACCESS_KEY_ID}"
: "${S3_SECRET_ACCESS_KEY:?missing S3_SECRET_ACCESS_KEY}"
: "${AGE_IDENTITY_FILE:?missing AGE_IDENTITY_FILE (path to private key, off-host)}"
: "${ASSERT_COLLECTION:=checkin_events}"
: "${ASSERT_MIN_COUNT:=1}"

prefix="${S3_PREFIX:-}"
prefix="${prefix%/}"
[[ -n "$prefix" ]] && prefix="${prefix}/"

scratch_db="bandao_restore_drill_$(date -u +%s)"

export AWS_ACCESS_KEY_ID="$S3_ACCESS_KEY_ID"
export AWS_SECRET_ACCESS_KEY="$S3_SECRET_ACCESS_KEY"
export AWS_DEFAULT_REGION="$S3_REGION"

# Find the most recent daily/<stamp>.archive.gz.age object.
latest=$(aws s3api list-objects-v2 \
  --bucket "$S3_BUCKET" \
  --prefix "${prefix}daily/" \
  --query 'sort_by(Contents,&LastModified)[-1].Key' \
  --output text)
if [[ -z "$latest" || "$latest" == "None" ]]; then
  echo "no daily backups found in s3://$S3_BUCKET/${prefix}daily/" >&2
  exit 70
fi

logger -t bandao-restore-drill "drill against $latest into $scratch_db"

# Restore into scratch DB, then count + drop.
trap 'mongosh "$MONGO_URI" --quiet --eval "db.getSiblingDB(\"$scratch_db\").dropDatabase()" >/dev/null 2>&1 || true' EXIT

aws s3 cp --no-progress "s3://$S3_BUCKET/$latest" - \
  | age -d -i "$AGE_IDENTITY_FILE" \
  | mongorestore --uri="$MONGO_URI" --gzip --archive --nsFrom="${MONGO_DB}.*" --nsTo="${scratch_db}.*"

count=$(mongosh "$MONGO_URI" --quiet --eval \
  "print(db.getSiblingDB('$scratch_db').getCollection('$ASSERT_COLLECTION').countDocuments({}))")

if (( count < ASSERT_MIN_COUNT )); then
  logger -t bandao-restore-drill "FAIL: $ASSERT_COLLECTION has $count docs (need >= $ASSERT_MIN_COUNT)"
  echo "drill failed: $ASSERT_COLLECTION count=$count expected>=$ASSERT_MIN_COUNT" >&2
  exit 1
fi

logger -t bandao-restore-drill "OK: $ASSERT_COLLECTION docs=$count from $latest"
echo "drill OK: $latest restored, $ASSERT_COLLECTION count=$count"
