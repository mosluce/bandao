## Context

The api (`api/`, Rust + axum, MongoDB driver) and admin-web (`admin-web/`, Nuxt 3 SPA) currently run only on localhost via the dev README flow. The Flutter app (`app/`) reaches the api directly with Bearer tokens stored in `flutter_secure_storage`. Mongo runs in `docker-compose.yml` for development; there is no production database.

The operator has settled on:
- **Zeabur** as the platform for stateless services (api, admin-web).
- A self-controlled VPS / NAS for **MongoDB**, joined to the same Tailscale tailnet as the api container.
- Two public hostnames on the same registrable domain (`ccmos.tw`), one per service.
- AWS S3 for off-site backup storage (credentials supplied as env vars).
- GitHub branch protection + Zeabur auto-deploy on `main` ("path 1").

The api already reads its production config surface from `BANDAO_*` env vars (`api/src/config.rs`). The admin-web bakes `NUXT_PUBLIC_API_BASE_URL` into the static bundle at build time (`admin-web/nuxt.config.ts`). Both services are reasonably 12-factor today; the work is operational, not architectural.

## Goals / Non-Goals

**Goals:**
- Reproducible production deployment of api + admin-web from `main` with zero manual ssh.
- Mongo reachable only via private network — no exposed `27017` on the public internet.
- Daily backups to S3 with documented restore procedure and a recurring drill.
- Cookie auth (admin-web ↔ api) and Bearer auth (mobile ↔ api) both work over the production hostnames.
- Health-checked, zero-downtime rolling deploys driven by Zeabur on every `main` push.
- One-time docs covering env vars, first deploy, rollback, and runbook for common ops.

**Non-Goals:**
- Staging environment. Only prod for MVP; staging can be added later if pain emerges.
- Multi-region or HA Mongo (single primary on one host is acceptable; backup is the recovery path).
- Migration tooling. Mongo schema is implicit; api creates its indexes idempotently on startup. No explicit migration runner is introduced.
- Replacing the existing GitHub Actions test workflows. They stay as the gate for branch protection.
- Pulling Mongo into Zeabur (e.g. Zeabur prebuilt MongoDB) — operator wants control.
- Automating DNS or branch-protection rules from this repo. Those are GitHub/registrar admin tasks, documented in tasks.md.

## Decisions

### Decision: Two subdomains under the same eTLD+1 (vs path-routing on one host)

`bandao-api.ccmos.tw` (api) and `bandao-admin.ccmos.tw` (admin-web). Both share eTLD+1 `ccmos.tw`, so admin-web fetch → api is **same-site cross-origin**.

- `SameSite=Lax` cookies set by the api are sent on admin-web's XHR/fetch (same-site), so cookie auth survives without `SameSite=None`.
- CORS still required because origins differ. api responds with `Access-Control-Allow-Origin: https://bandao-admin.ccmos.tw` and `Access-Control-Allow-Credentials: true`. The existing `BANDAO_ALLOWED_ORIGIN` env var already controls this.
- Cookies are **host-only** (no explicit `Domain` attribute), restricting them to `bandao-api.ccmos.tw`. This avoids cookie leakage to other ccmos.tw services.
- **Alternative considered**: single host with `bandao.ccmos.tw/api/*` reverse-proxied. Cleaner cookies, no CORS — but Zeabur services are independent and adding a reverse proxy in front (Caddy, Cloudflare worker) is more moving parts than CORS already gives us. Rejected.

### Decision: Tailscale userspace networking inside the api container

The api container joins the operator's tailnet via `tailscale` running in userspace mode (`--tun=userspace-networking --socks5-server=localhost:1055`). The Mongo URI uses the Mongo host's MagicDNS name (e.g. `mongo.<tailnet>.ts.net`).

- **Why userspace**: Zeabur containers cannot get a TUN device. Userspace mode runs entirely in the container's PID namespace.
- **Auth key**: a tagged, ephemeral-disabled, **reusable** auth key from Tailscale admin console, stored as `TS_AUTHKEY` Zeabur env var. ACL tag like `tag:bandao-api` so Mongo host's tailnet ACL can scope inbound to that tag only.
- **Failure mode**: if Tailscale fails to come up, api startup fails fast (the Mongo connect will time out) and Zeabur health probe keeps the previous revision serving. This is preferred over silent fallback to public Mongo.
- **Alternative considered**: Cloudflare Tunnel running on the Mongo host fronting Mongo wire protocol. Tunneling raw TCP works but Cloudflare's free TCP tunnel pricing is unclear and the dev experience for Mongo wire over `cloudflared` is less battle-tested than Tailscale for Mongo. Rejected for MVP.
- **Alternative considered**: public Mongo + TLS + strong auth + fail2ban. Simplest, but largest attack surface; one Mongo CVE away from incident. Rejected.

### Decision: Single Dockerfile per service, multi-stage

`api/Dockerfile`: builder stage uses `rust:1.<pinned>-slim` (matches `rust-toolchain.toml`), runs `cargo build --release --locked`, then a runtime stage based on `debian:stable-slim` with only the `bandao-api` binary, CA certificates, and the Tailscale binary. Distroless considered but skipped because Tailscale binary ships glibc.

`admin-web/Dockerfile` (or Zeabur Nuxt preset): build stage runs `pnpm install --frozen-lockfile && pnpm build`, runtime stage is `nginx:alpine` serving `.output/public` with SPA fallback (`try_files $uri /index.html`). If Zeabur's Nuxt detector handles SPA fallback for `ssr:false` natively, prefer that and skip the Dockerfile.

### Decision: api `/healthz` endpoint, no DB hit

`GET /healthz` returns `200 OK` with body `{"status":"ok"}` immediately. Does **not** ping Mongo. Reason: a deeper `/readyz` that pings Mongo is desirable for SLO/observability later, but for Zeabur's basic health probe the goal is "process is up and listening". Conflating health with Mongo readiness causes false-positive deploy failures during transient network blips.

A future change can add `/readyz` (Mongo ping + Tailscale up check) when monitoring lands.

### Decision: CI/CD path 1 — branch protection + Zeabur auto-deploy

- Existing workflows stay: `.github/workflows/api.yml`, `admin-web.yml`, `app.yml`.
- `main` branch protection (configured in GitHub UI, documented in tasks.md):
  - Require pull request before merging.
  - Require status checks `api / fmt + clippy + test`, `admin-web / typecheck + test + build`, `app / ...` to pass.
  - Require linear history (no merge commits).
  - Block force pushes and deletions.
- Zeabur GitHub integration watches `main`. On push, it rebuilds whichever service's path changed (`api/` or `admin-web/`) and rolls out behind its health probe.
- **No deploy workflow** in this repo. Rationale: keeps repo free of Zeabur-specific config, no token to rotate, no race between CI and deploy. The trade-off is that a pushed-but-CI-broken commit on `main` would still deploy — branch protection prevents that path.

### Decision: Backup pipeline runs on the Mongo host, not Zeabur

A systemd-timer or cron on the Mongo host runs daily:
1. `mongodump --gzip --archive=...` against localhost.
2. Encrypts with `age` or `gpg` using a public key whose private key lives off-host.
3. Uploads to S3 via `aws s3 cp` using `S3_ACCESS_KEY_ID` / `S3_SECRET_ACCESS_KEY` from a root-only env file.
4. Retains S3 lifecycle policy: daily for 30 days, weekly for 12 weeks, monthly for 12 months.

Restore drill (monthly): pull most recent dump, decrypt, restore into a scratch DB, run a count assertion against a known collection, drop the scratch DB. Drill failure pages the operator.

- **Why not Zeabur cron**: Zeabur has no direct Mongo access from cron without joining the tailnet too — better to keep backup colocated with the data source.
- **Why encrypt**: S3 IAM is tight, but encryption-at-rest under operator-controlled key adds defense-in-depth against bucket misconfig.

### Decision: First-deploy bootstrap uses the existing register flow

The operator manually visits admin-web in a browser, uses the existing register / org-creation UI to create the first Org and admin account. No seed script. Rationale: register flow already exists and is exercised by tests; adding a separate prod-only seed path doubles the surface to maintain.

This also implies the api and admin-web must be reachable on their public hostnames before bootstrap — no ordering surprise, since deploy → DNS → bootstrap is the natural sequence.

## Risks / Trade-offs

- **[Risk] Mongo host is a single point of failure.** → Mitigation: daily encrypted backups, monthly restore drill, host-level monitoring out of scope for this change but tracked as a follow-up.
- **[Risk] Tailscale auth key rotation is manual.** → Mitigation: use a reusable, non-ephemeral key tagged for this service so a rotation just means setting a new `TS_AUTHKEY` env var on Zeabur and redeploying. Document in DEPLOY.md.
- **[Risk] Zeabur Rust build cold start is slow (10+ min cargo build).** → Mitigation: rely on Zeabur's BuildKit layer cache; pin `rust-toolchain.toml`. If this becomes painful, switch to GH Actions building a container image and pushing to GHCR with Zeabur pulling — but only after observing the pain.
- **[Risk] CORS misconfigured causes silent admin-web breakage in prod.** → Mitigation: smoke step in tasks.md explicitly covers admin-web login over the prod URL pair before declaring done.
- **[Risk] S3 credentials leak via env var introspection on Mongo host.** → Mitigation: scope IAM user to one bucket with `s3:PutObject` + `s3:GetObject` + lifecycle policy only; no `*` actions; no console access.
- **[Trade-off] No staging.** → If a deploy breaks prod, mitigation is `git revert` + push. This is acceptable for MVP velocity.
- **[Trade-off] No SSR for admin-web.** → Already an established project decision (`ssr: false`). Static deploy is simpler and removes one runtime to operate.
- **[Trade-off] Health probe is shallow.** → Possible to deploy a build that starts listening but cannot reach Mongo. Symptom would be 5xx on real requests; surfaced quickly via app/admin-web error reports. Deeper readiness check is a future change.

## Migration Plan

1. Pre-deploy: provision Mongo host, install Tailscale, install mongod 7.x, create user/password, restrict bind address to localhost + tailnet interface only.
2. Provision Tailscale auth key with appropriate ACL tags.
3. Provision AWS S3 bucket + IAM user; capture access/secret keys.
4. Add `api/Dockerfile`, optional `admin-web/Dockerfile`, api `/healthz` handler, `.dockerignore` files in a PR; merge once CI green.
5. Create Zeabur project; connect GitHub repo; configure two services pointed at `api/` and `admin-web/`; set env vars per `DEPLOY.md`; deploy.
6. Add CNAME records for both subdomains pointing at Zeabur targets; wait for TLS issue.
7. Configure GitHub branch protection on `main`.
8. Configure Mongo-host backup cron + run first dump → S3 manually to verify.
9. Smoke: register first Org via admin-web; log in; create checkin; verify mobile app login + checkin against `bandao-api.ccmos.tw`.
10. Document everything (`DEPLOY.md`).

**Rollback**: at the platform layer, Zeabur keeps prior revisions and offers one-click rollback per service. Application-level rollback is `git revert` of the offending commit + push, which Zeabur picks up automatically. No state migration needed for a code rollback.

## Open Questions

- Does Zeabur's Nuxt auto-detect handle SPA fallback for `ssr:false`? If not, ship a tiny `nginx:alpine` Dockerfile for admin-web. Resolved during step 5 of the migration plan.
- Should backup encryption use `age` (simpler) or `gpg` (more familiar)? Operator preference; default is `age` unless objection.
- AWS region for the S3 bucket. Default to `ap-northeast-1` (Tokyo) for low latency from a Taiwan-based Mongo host unless operator specifies otherwise.
