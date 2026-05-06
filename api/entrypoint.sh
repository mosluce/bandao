#!/usr/bin/env bash
set -euo pipefail

if [[ -n "${TS_AUTHKEY:-}" ]]; then
  echo "[entrypoint] starting tailscaled (userspace networking)"
  tailscaled --tun=userspace-networking --state=mem: --socks5-server=localhost:1055 &

  echo "[entrypoint] tailscale up"
  tailscale up \
    --authkey="${TS_AUTHKEY}" \
    --hostname="${TS_HOSTNAME:-bandao-api}" \
    --accept-routes \
    --accept-dns=true
else
  echo "[entrypoint] TS_AUTHKEY unset — skipping tailscale; api will use direct network"
fi

exec /usr/local/bin/bandao-api
