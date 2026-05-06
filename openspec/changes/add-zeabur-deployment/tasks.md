## 1. Pre-flight (operator-only, off-repo)

- [x] 1.1 Create / pick the Mongo host (VPS or NAS the operator can root-SSH into); record its public IP and OS version.
- [x] 1.2 Sign up for Tailscale with the operator's Google/GitHub identity; create a tailnet ACL tag `tag:bandao-api` and `tag:bandao-mongo`; mint a reusable, non-ephemeral auth key tagged `tag:bandao-api` for the api container.
- [x] 1.3 Create an AWS S3 bucket dedicated to backups (e.g. `bandao-mongo-backups-<region>`); create an IAM user with a policy scoped to `s3:PutObject`, `s3:GetObject`, `s3:ListBucket` on that bucket only; capture access/secret keys.
- [x] 1.4 Configure the S3 bucket lifecycle rules: keep daily prefix for 30 days, weekly for 12 weeks, monthly for 12 months; enable versioning and default encryption (SSE-S3).
- [x] 1.5 Generate a backup encryption keypair (recommend `age` — `age-keygen`); store private key off-host in the operator's password manager; the public key goes on the Mongo host for encryption.
- [x] 1.6 Decide AWS region (default `ap-northeast-1`) and write decisions to the runbook draft.

## 2. api repo work — Dockerfile + healthz

- [x] 2.1 Add `api/.dockerignore` excluding `target/`, `tests/snapshots/`, IDE files.
- [x] 2.2 Add `api/Dockerfile` as a multi-stage build: builder stage based on a Rust image matching `rust-toolchain.toml`, runtime stage based on `debian:stable-slim` containing `bandao-api`, `ca-certificates`, and the Tailscale binary.
- [x] 2.3 In the runtime image, install Tailscale via the official Debian repo or copy a pinned `tailscale` + `tailscaled` static binary; set up an entrypoint script that runs `tailscaled --tun=userspace-networking --state=mem:` in the background, runs `tailscale up --authkey=$TS_AUTHKEY --hostname=bandao-api --accept-routes`, then exec's `bandao-api`.
- [x] 2.4 Add `GET /healthz` to the api: register the route in `api/src/handlers/` (or wherever the existing router is built) returning `200 OK` with `{"status":"ok"}` and no Mongo access; ensure the route bypasses auth middleware.
- [x] 2.5 Add an api integration test asserting `/healthz` returns 200 with the expected JSON when the app is built with no Mongo configured (or with Mongo disconnected).
- [x] 2.6 Verify `cargo fmt`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test` all pass locally.

## 3. admin-web repo work — production build path

- [x] 3.1 Confirm `nuxt.config.ts` already produces SPA output (`ssr: false`) and `pnpm build` emits `.output/public`. No code change expected.
- [x] 3.2 Add `admin-web/.dockerignore` excluding `node_modules/`, `.nuxt/`, `.output/`, `dist/`, IDE files. (Required only if step 3.3 ships a Dockerfile.)
- [x] 3.3 If Zeabur's Nuxt auto-detect does not handle SPA fallback (`/foo` → `/index.html`), add `admin-web/Dockerfile` as a two-stage build: build with `node:20-alpine` + `pnpm`, runtime with `nginx:alpine` serving `/usr/share/nginx/html` and a config that does `try_files $uri $uri/ /index.html;`.
- [x] 3.4 Smoke `pnpm install --frozen-lockfile && pnpm build && pnpm preview` locally and walk through the SPA in a browser to catch any build-time env coupling (e.g. `NUXT_PUBLIC_API_BASE_URL`).

## 4. Mongo host — base setup

- [x] 4.1 Install MongoDB 7.x from the official MongoDB apt repo following MongoDB's docs for the host's OS.
- [x] 4.2 Create a strong admin user and a dedicated `bandao` database user with only `readWrite` on the `bandao` database (avoid root for the app's connection).
- [x] 4.3 Configure `/etc/mongod.conf` `net.bindIp` to bind only to `127.0.0.1` and the tailnet interface (e.g. `tailscale0`); restart and verify `ss -lntp` shows no `0.0.0.0:27017`.
- [x] 4.4 Install Tailscale on the Mongo host; bring it up with a tag of `tag:bandao-mongo`; verify the host is visible in the tailnet admin UI.
- [x] 4.5 In the Tailscale admin ACL, restrict `tag:bandao-api` to reach `tag:bandao-mongo` only on TCP/27017; deny everything else.
- [x] 4.6 From a separate tailnet device, run `mongosh "mongodb://bandao:<pass>@<mongo-host>.<tailnet>.ts.net:27017/bandao"` and confirm a successful connection.

## 5. Mongo host — backup pipeline

- [x] 5.1 Install `awscli` v2 (or `rclone`) and `age` on the Mongo host.
- [x] 5.2 Create `/etc/bandao-backup.env` (mode `0600`, owned by root) holding `S3_ACCESS_KEY_ID`, `S3_SECRET_ACCESS_KEY`, `S3_BUCKET`, `S3_REGION`, and the `age` recipient public key.
- [x] 5.3 Write a backup script (e.g. `/usr/local/bin/bandao-backup.sh`) that: sources the env file, runs `mongodump --gzip --archive=- | age -r <recipient> | aws s3 cp - s3://$S3_BUCKET/daily/$(date +%Y-%m-%d).age`, exits non-zero on any pipeline failure (`set -euo pipefail` and `pipefail`), and logs to syslog.
- [x] 5.4 Add a systemd timer (or cron) to run the backup script daily at a low-traffic local time.
- [x] 5.5 Trigger the backup script manually once; verify the encrypted archive lands in S3 under `daily/<date>.age` and is non-empty.
- [x] 5.6 Write a restore-drill script (`/usr/local/bin/bandao-restore-drill.sh`) that: pulls the latest `daily/*.age` from S3, decrypts with the operator's private key (provided interactively or via SOPS / SSH-agent — never stored on the host), pipes to `mongorestore` into a scratch DB, runs `db.<known-collection>.countDocuments()` and asserts it exceeds a baseline, then drops the scratch DB.
- [x] 5.7 Run the restore drill manually; confirm pass; document the procedure for the monthly cadence (operator's calendar).

## 6. Zeabur project setup

- [x] 6.1 Create a Zeabur project named `bandao`; connect it to the GitHub repo `mosluce/bandao` with permissions for `main` only.
- [x] 6.2 Add the `api` service: source path `api/`, build via Dockerfile; set env vars per `DEPLOY.md` matrix: `BANDAO_LISTEN_ADDR=0.0.0.0:8080`, `BANDAO_MONGO_URI=mongodb://bandao:<pass>@<mongo-host>.<tailnet>.ts.net:27017/bandao?authSource=admin`, `BANDAO_MONGO_DB=bandao`, `BANDAO_COOKIE_SECURE=true`, `BANDAO_ALLOWED_ORIGIN=https://bandao-admin.ccmos.tw`, `BANDAO_SESSION_TTL_SECONDS=1209600`, `TS_AUTHKEY=<auth key from 1.2>`. Leave `BANDAO_COOKIE_DOMAIN` unset (host-only).
- [x] 6.3 Add the `admin-web` service: source path `admin-web/`, build via Nuxt auto-detect (or the Dockerfile from 3.3); set build-time env: `NUXT_PUBLIC_API_BASE_URL=https://bandao-api.ccmos.tw`.
- [x] 6.4 Configure Zeabur health check on the `api` service: `GET /healthz` expecting `200`, with sane interval/timeout (e.g. 15s / 5s).
- [x] 6.5 Trigger the first deploy of both services; observe build logs; confirm api comes up and `/healthz` returns 200; confirm admin-web's index document loads.

## 7. DNS + TLS

- [x] 7.1 In the `ccmos.tw` registrar / DNS provider, add a CNAME record for `bandao-api.ccmos.tw` pointing at the Zeabur target host shown on the api service.
- [x] 7.2 Add a CNAME record for `bandao-admin.ccmos.tw` pointing at the Zeabur target for admin-web.
- [x] 7.3 In Zeabur, attach both custom domains to their respective services; wait for the automated TLS issuance to complete; verify both hostnames serve a valid public-CA certificate via `curl -vI https://bandao-api.ccmos.tw/healthz` and `curl -vI https://bandao-admin.ccmos.tw/`.
- [x] 7.4 Verify HTTP requests to either hostname are upgraded or refused (e.g. `curl -I http://bandao-api.ccmos.tw/healthz` returns a 301/308 to https or fails the connection).

## 8. GitHub branch protection

- [x] 8.1 In repo settings, add a branch protection rule for `main`: require pull request before merging; require approvals = 1 (or 0 if a solo project, operator's call but document the choice).
- [x] 8.2 Require the following status checks to pass: `api / fmt + clippy + test`, `admin-web / typecheck + test + build`, `app / analyze + test`.
- [x] 8.3 Require linear history; block force pushes; block branch deletions.
- [x] 8.4 Verify by opening a draft PR with a deliberately failing test and confirming the merge button is disabled.

## 9. End-to-end smoke

- [x] 9.1 From a browser, register the first Org and admin account on `https://bandao-admin.ccmos.tw` using the existing register UI; record the credentials in the operator's password manager.
- [x] 9.2 Log in to admin-web with those credentials; open DevTools network tab; verify the login request hits `https://bandao-api.ccmos.tw`, the response sets a cookie with `Secure`, `HttpOnly`, `SameSite=Lax`, no `Domain` attribute; verify a follow-up authenticated call carries the cookie and succeeds (200, not 401).
- [x] 9.3 From the mobile app build pointed at `https://bandao-api.ccmos.tw`, register or log in an AppUser; perform an on-duty checkin; verify the checkin appears in admin-web's checkin dashboard within polling latency.
- [x] 9.4 From any other origin (e.g. a scratch HTML page on a different domain), issue a credentialed `fetch` to the api login endpoint and confirm the browser blocks the response (CORS rejects the unrelated origin).
- [x] 9.5 Restart the api Zeabur service via the dashboard; confirm zero-downtime rollout (admin-web does not visibly fail mid-rollout) and that `/healthz` is green throughout.

## 10. Documentation

- [x] 10.1 Create `DEPLOY.md` at the repo root containing: architecture diagram (api / admin-web / Mongo / Tailscale / S3), env var matrix per service, first-deploy bootstrap, Tailscale auth-key rotation steps, S3 backup verification + restore drill, rollback procedures, and a smoke-test checklist mirroring section 9.
- [x] 10.2 Add a short pointer in `README.md` and `AGENTS.md` directing readers to `DEPLOY.md` for production ops.
- [x] 10.3 Update `ROADMAP.md` to remove any deployment-related side ideas now covered by this change, and to track follow-ups (e.g. `/readyz`, monitoring, staging) as new ideas.

## 11. Hand-off

- [ ] 11.1 Run `openspec validate add-zeabur-deployment` to confirm the change passes structural checks before merging.
- [ ] 11.2 Open a single PR titled `chore(infra): add zeabur deployment` containing the api Dockerfile, optional admin-web Dockerfile, healthz handler + test, and `DEPLOY.md`; link the OpenSpec change in the PR body.
- [ ] 11.3 After merge + first successful production deploy + green smoke (section 9), run `/opsx:archive add-zeabur-deployment` to archive the change and sync specs.
