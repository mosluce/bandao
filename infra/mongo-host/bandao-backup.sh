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
#   S3_PREFIX       prefix inside the bucket (default "")
#
# Schedule via the bundled systemd timer (bandao-backup.timer) at a
# low-traffic local hour. Logs go to syslog.

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

prefix="${S3_PREFIX:-}"
prefix="${prefix%/}"
[[ -n "$prefix" ]] && prefix="${prefix}/"

stamp=$(date -u +%Y-%m-%dT%H-%M-%SZ)
key="${prefix}daily/${stamp}.archive.gz.age"

export AWS_ACCESS_KEY_ID="$S3_ACCESS_KEY_ID"
export AWS_SECRET_ACCESS_KEY="$S3_SECRET_ACCESS_KEY"
export AWS_DEFAULT_REGION="$S3_REGION"

logger -t bandao-backup "starting dump db=$MONGO_DB → s3://$S3_BUCKET/$key"

mongodump --uri="$MONGO_URI" --db="$MONGO_DB" --gzip --archive \
  | age -r "$AGE_RECIPIENT" \
  | aws s3 cp --no-progress - "s3://$S3_BUCKET/$key"

logger -t bandao-backup "uploaded s3://$S3_BUCKET/$key"
