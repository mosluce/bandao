#!/usr/bin/env bash
#
# Programmatically reset an AppUser's password against the production api.
# Mirrors what the in-app force-change-password screen does — login →
# POST /app/me/password — but without bouncing through the Flutter UI.
# Useful for clearing the `needs_password_change` flag on demo / seed
# accounts before handing the credentials to App Store / TestFlight
# reviewers.
#
# Usage:
#   ./scripts/reset_appuser_password.sh \
#     --org-code  demo \
#     --username  demo \
#     --current   JJEDP9QXBYT4 \
#     --new       <new password, ≥8 chars>
#
# Or via env: BANDAO_RESET_ORG / _USER / _CURRENT / _NEW.
#
# Exits 0 on success. Verifies by re-logging in with the new password.

set -euo pipefail

ORG="${BANDAO_RESET_ORG:-}"
USERNAME="${BANDAO_RESET_USER:-}"
CURRENT="${BANDAO_RESET_CURRENT:-}"
NEW="${BANDAO_RESET_NEW:-}"
API_URL="${BANDAO_API_URL:-https://bandao-api.ccmos.tw}"

usage() {
  cat <<'USAGE'
Usage: reset_appuser_password.sh [options]

Required (also accept env vars: BANDAO_RESET_ORG / _USER / _CURRENT / _NEW):
  --org-code   CODE     AppUser's Org code.
  --username   NAME     AppUser username.
  --current    PASS     Current password (the temporary one).
  --new        PASS     New password (>= 8 characters).

Optional:
  --api        URL      API base URL (default: https://bandao-api.ccmos.tw,
                        env BANDAO_API_URL).
  -h, --help            Print this help.
USAGE
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --org-code) ORG="$2"; shift 2 ;;
    --username) USERNAME="$2"; shift 2 ;;
    --current)  CURRENT="$2"; shift 2 ;;
    --new)      NEW="$2"; shift 2 ;;
    --api)      API_URL="$2"; shift 2 ;;
    -h|--help)  usage; exit 0 ;;
    *) echo "Unknown option: $1" >&2; usage; exit 2 ;;
  esac
done

if [[ -z "$ORG" || -z "$USERNAME" || -z "$CURRENT" || -z "$NEW" ]]; then
  echo "Missing required arguments." >&2
  usage
  exit 2
fi

if [[ "${#NEW}" -lt 8 ]]; then
  echo "--new must be at least 8 characters (api enforces MIN_PASSWORD_LEN=8)." >&2
  exit 2
fi

# `python3 -c json.dumps(...)` to escape special chars cleanly in the JSON
# body (the password might contain quote / backslash / unicode).
json_str() {
  python3 -c 'import json, sys; print(json.dumps(sys.argv[1]))' "$1"
}

# ── Login ───────────────────────────────────────────────────────────────
echo "──▶ POST $API_URL/app/auth/login"
LOGIN_BODY=$(cat <<EOF
{
  "org_code": $(json_str "$ORG"),
  "username": $(json_str "$USERNAME"),
  "password": $(json_str "$CURRENT")
}
EOF
)

LOGIN_RESP=$(curl -sS -X POST \
  -H "Content-Type: application/json" \
  -d "$LOGIN_BODY" \
  "$API_URL/app/auth/login")

TOKEN=$(printf '%s' "$LOGIN_RESP" | python3 -c '
import json, sys
d = json.load(sys.stdin)
if "token" not in d:
    sys.stderr.write("Login response had no token. Body:\n" + json.dumps(d, ensure_ascii=False, indent=2) + "\n")
    sys.exit(1)
print(d["token"])
')

if [[ -z "$TOKEN" ]]; then
  echo "Login failed — no token in response." >&2
  echo "Body: $LOGIN_RESP" >&2
  exit 1
fi

echo "    ✓ token acquired"

# ── Change password ────────────────────────────────────────────────────
echo "──▶ POST $API_URL/app/me/password"
CHANGE_BODY=$(cat <<EOF
{
  "current_password": $(json_str "$CURRENT"),
  "new_password": $(json_str "$NEW")
}
EOF
)

HTTP_CODE=$(curl -sS -o /tmp/bandao-pwchg.body -w "%{http_code}" -X POST \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d "$CHANGE_BODY" \
  "$API_URL/app/me/password")

if [[ "$HTTP_CODE" != "204" ]]; then
  echo "Password change failed with HTTP $HTTP_CODE" >&2
  echo "Body:" >&2
  cat /tmp/bandao-pwchg.body >&2
  echo >&2
  exit 1
fi

echo "    ✓ password changed (204 No Content)"

# ── Verify by re-logging in with the new password ──────────────────────
echo "──▶ Verify: POST $API_URL/app/auth/login (with new password)"
VERIFY_BODY=$(cat <<EOF
{
  "org_code": $(json_str "$ORG"),
  "username": $(json_str "$USERNAME"),
  "password": $(json_str "$NEW")
}
EOF
)

VERIFY_HTTP=$(curl -sS -o /tmp/bandao-verify.body -w "%{http_code}" -X POST \
  -H "Content-Type: application/json" \
  -d "$VERIFY_BODY" \
  "$API_URL/app/auth/login")

if [[ "$VERIFY_HTTP" != "200" ]]; then
  echo "Verification login failed with HTTP $VERIFY_HTTP — password change " >&2
  echo "may have succeeded but the new password isn't accepted." >&2
  cat /tmp/bandao-verify.body >&2
  exit 1
fi

# Pretty-print whether needs_password_change is now false.
NEEDS=$(python3 -c '
import json
d = json.load(open("/tmp/bandao-verify.body"))
print(d.get("needs_password_change"))
')
echo "    ✓ verify login succeeded; needs_password_change = $NEEDS"

rm -f /tmp/bandao-pwchg.body /tmp/bandao-verify.body

echo
echo "Done. Hand the new password to App Store / TestFlight reviewers."
