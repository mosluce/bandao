#!/bin/bash
# Build the legacy_backfill example and publish it, its .env, and its runner
# into the LaunchAgent's deploy directory.
#
# The deploy directory exists because launchd cannot read the repo on the
# external /Volumes/Backup disk (macOS TCC denies it — `Operation not
# permitted`). Everything the hourly job touches therefore has to live under
# $HOME. Re-run this after any rebuild or .env change, otherwise the scheduled
# job keeps using the old copy.
set -euo pipefail

API_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DEPLOY_DIR="$HOME/Library/Application Support/bandao-legacy-backfill"
CARGO="${CARGO:-$HOME/.cargo/bin/cargo}"

cd "$API_DIR"

echo "building release example…"
"$CARGO" build --release --example legacy_backfill

mkdir -p "$DEPLOY_DIR"
install -m 755 "$API_DIR/target/release/examples/legacy_backfill" "$DEPLOY_DIR/legacy_backfill"
install -m 755 "$API_DIR/scripts/legacy_backfill_run.sh" "$DEPLOY_DIR/run.sh"
install -m 600 "$API_DIR/.env" "$DEPLOY_DIR/.env"

echo "synced to $DEPLOY_DIR"
ls -la "$DEPLOY_DIR"
