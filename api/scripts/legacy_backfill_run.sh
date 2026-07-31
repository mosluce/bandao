#!/bin/bash
# Hourly incremental legacy backfill, driven by the
# io.no8.bandao.legacy-backfill LaunchAgent (08:00–19:00 local, hourly).
#
# This script runs from the deploy directory
# (~/Library/Application Support/bandao-legacy-backfill), NOT from the repo:
# launchd-spawned processes are denied access to the external /Volumes/Backup
# disk by macOS TCC, so the binary and .env are copied there by
# scripts/legacy_backfill_sync.sh. Re-run this sync after every rebuild or
# .env change.
#
# Re-running is a no-op for rows already imported (partial unique index on
# legacy_source_id), which is what makes an overlapping --since-days 1 window
# safe.
set -uo pipefail

DEPLOY_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$DEPLOY_DIR" || exit 1

# Read from .env rather than hardcoded: this repository is public, and a
# literal org id here would pin one customer's deployment into it. The sync
# script already copies .env into the deploy dir, so nothing extra is needed
# beyond a LEGACY_BACKFILL_ORG_ID entry there.
ORG_ID="${LEGACY_BACKFILL_ORG_ID:-}"
SINCE_DAYS=1
BIN="$DEPLOY_DIR/legacy_backfill"
LOCK_DIR="$DEPLOY_DIR/.lock"

log() { printf '[%s] %s\n' "$(date '+%Y-%m-%d %H:%M:%S')" "$*"; }

# mkdir is atomic — a still-running previous hour keeps the lock and this run
# exits instead of racing it.
if ! mkdir "$LOCK_DIR" 2>/dev/null; then
    log "skip: another run holds $LOCK_DIR"
    exit 0
fi
trap 'rmdir "$LOCK_DIR" 2>/dev/null' EXIT

if [ ! -x "$BIN" ] || [ ! -f "$DEPLOY_DIR/.env" ]; then
    log "deploy dir incomplete (missing binary or .env) — run api/scripts/legacy_backfill_sync.sh"
    exit 1
fi

if [ -z "$ORG_ID" ]; then
    ORG_ID="$(sed -n 's/^LEGACY_BACKFILL_ORG_ID=//p' "$DEPLOY_DIR/.env" \
        | tail -1 | tr -d '"'"'"'"'"'"' \r')"
fi
if [ -z "$ORG_ID" ]; then
    log "no org id: set LEGACY_BACKFILL_ORG_ID in api/.env, then re-run api/scripts/legacy_backfill_sync.sh"
    exit 1
fi

log "start --org-id $ORG_ID --since-days $SINCE_DAYS"
"$BIN" --org-id "$ORG_ID" --since-days "$SINCE_DAYS"
status=$?
log "done (exit $status)"
exit $status
