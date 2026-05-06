# Production deployment runbook

Production for **班到 (bandao)** runs on Zeabur for the stateless services
(`api`, `admin-web`) and on an operator-controlled host for MongoDB. The
api reaches Mongo only over a Tailscale private network; backups land in
AWS S3.

This file is the authoritative runbook. Changes to production topology,
env vars, or operational procedures should land here in the same PR.

## Architecture

```
                ┌────────────────────────── public internet ──────────────────────────┐
                │                                                                     │
   browsers ───▶│  https://bandao-admin.ccmos.tw   (admin-web, Nuxt SPA, Zeabur)      │
   mobile   ───▶│  https://bandao-api.ccmos.tw     (api, Rust binary, Zeabur)         │
                │                              │                                      │
                │                              ▼                                      │
                │                      Tailscale tailnet (private)                    │
                │                              │                                      │
                └──────────────────────────────┼──────────────────────────────────────┘
                                               ▼
                                  Mongo host (operator-controlled)
                                  ├── mongod 7.x (bind 127.0.0.1 + tailscale0)
                                  ├── tailscaled
                                  └── bandao-backup.timer ─▶ AWS S3 (encrypted dumps)
```

- `bandao-admin.ccmos.tw` and `bandao-api.ccmos.tw` are siblings under the
  same registrable domain (`ccmos.tw`). They are **same-site cross-origin**,
  so cookie auth survives with `SameSite=Lax`.
- The api accepts both **cookie auth** (admin-web) and **Bearer token auth**
  (mobile app) on the same host.
- Mongo is **never** exposed to the public internet. Only nodes carrying
  the `tag:bandao-api` Tailscale ACL tag can reach `:27017` on the Mongo
  host.

## Repositories and ownership

- `api/` — Rust binary; `Dockerfile` + `entrypoint.sh` ship the image.
- `admin-web/` — Nuxt 3 SPA (`ssr: false`); `Dockerfile` is provided as a
  fallback (nginx serving `.output/public` with SPA fallback) in case
  Zeabur's Nuxt auto-detect does not handle SPA routing.
- `infra/mongo-host/` — backup scripts + systemd units to be installed on
  the Mongo host. See [`infra/mongo-host/README.md`](./infra/mongo-host/README.md).
- `openspec/changes/add-zeabur-deployment/` — proposal, design, specs, tasks
  for the change that introduced this runbook.

## Environment variables

### `api` service on Zeabur

| Var | Required | Production value | Notes |
| --- | --- | --- | --- |
| `BANDAO_LISTEN_ADDR` | yes | `0.0.0.0:8080` | Default `127.0.0.1:8080` only binds loopback. |
| `BANDAO_MONGO_URI` | yes | `mongodb://bandao:<pw>@<mongo-host>.<tailnet>.ts.net:27017/bandao?authSource=admin` | Use the Tailscale MagicDNS hostname, not an IP. |
| `BANDAO_MONGO_DB` | yes | `bandao` | |
| `BANDAO_COOKIE_SECURE` | yes | `true` | Production runs over HTTPS only. |
| `BANDAO_COOKIE_DOMAIN` | no | _(unset)_ | Leave unset → host-only cookie on `bandao-api.ccmos.tw`. |
| `BANDAO_ALLOWED_ORIGIN` | yes | `https://bandao-admin.ccmos.tw` | Single origin. CORS reflects this exact value. |
| `BANDAO_SESSION_TTL_SECONDS` | no | `1209600` (14 days) | Adjust per security policy. |
| `TS_AUTHKEY` | yes | _(reusable Tailscale auth key)_ | Tagged `tag:bandao-api`. Rotate on operator's schedule. |
| `TS_HOSTNAME` | no | `bandao-api` | Container's tailnet hostname. |

The api refuses to start if `BANDAO_LISTEN_ADDR` cannot parse. Other vars
fall back to the dev defaults baked into `api/src/config.rs` — production
SHOULD set every row above explicitly.

### `admin-web` service on Zeabur

| Var | Required | Production value | Notes |
| --- | --- | --- | --- |
| `NUXT_PUBLIC_API_BASE_URL` | yes (build-time) | `https://bandao-api.ccmos.tw` | Baked into the static bundle at `pnpm build`. Changing it requires a rebuild. |

If using the bundled `admin-web/Dockerfile`, pass via Docker build arg:
`--build-arg NUXT_PUBLIC_API_BASE_URL=https://bandao-api.ccmos.tw`. Zeabur's
Nuxt auto-detect injects build-time env from the service's environment
section.

### Mongo host (`/etc/bandao-backup.env`, mode 0600 root-owned)

| Var | Required | Notes |
| --- | --- | --- |
| `MONGO_URI` | yes | A connection string with a user that has the `backup` role. |
| `MONGO_DB` | yes | `bandao` |
| `AGE_RECIPIENT` | yes | `age1...` public key. The matching private key lives off-host. |
| `S3_BUCKET` | yes | Dedicated bucket. |
| `S3_REGION` | yes | e.g. `ap-northeast-1`. |
| `S3_ACCESS_KEY_ID` | yes | IAM user scoped to this bucket. |
| `S3_SECRET_ACCESS_KEY` | yes | |
| `S3_PREFIX` | no | Optional prefix inside the bucket. |

Never commit any of these values. Examples in `infra/mongo-host/README.md`
use placeholders.

## First-time bootstrap

Order matters. Each step has a verifiable acceptance criterion.

1. **Provision the Mongo host.** Pick a VPS / NAS the operator can root-SSH
   into. Install `mongod` 7.x, `tailscale`, `awscli` v2, `age`, and
   `mongodb-database-tools`. Bind `mongod` to `127.0.0.1` and the
   `tailscale0` interface only — verify with `ss -lntp`. ✓ when no
   `0.0.0.0:27017` line appears.

2. **Bring up Tailscale on the Mongo host.** `sudo tailscale up
   --advertise-tags=tag:bandao-mongo`. ✓ when the host appears in the
   tailnet admin UI.

3. **Mint a Tailscale auth key for the api container.** Reusable,
   non-ephemeral, tagged `tag:bandao-api`. Store as the Zeabur service env
   `TS_AUTHKEY`. Add an ACL rule allowing `tag:bandao-api → tag:bandao-mongo`
   on `tcp:27017` only.

4. **Provision the S3 backup bucket + IAM user.** Permissions limited to
   `s3:PutObject`, `s3:GetObject`, `s3:ListBucket` on the single bucket.
   Apply the lifecycle JSON in `infra/mongo-host/README.md`. Keep
   versioning + SSE-S3 default encryption on.

5. **Generate the `age` keypair on the operator workstation** (NOT on the
   host). Save the private key to the password manager; copy only the
   public key into `/etc/bandao-backup.env`.

6. **Install backup scripts + timer** per
   [`infra/mongo-host/README.md`](./infra/mongo-host/README.md). Trigger
   the first run manually; verify the encrypted archive lands in S3.

7. **Add the api service on Zeabur.** Source path `api/`, build with the
   bundled `Dockerfile`. Set every env var in the api table above. Configure
   the health check to `GET /healthz`.

8. **Add the admin-web service on Zeabur.** Source path `admin-web/`,
   either the Nuxt auto-detect or the bundled `Dockerfile`. Set
   `NUXT_PUBLIC_API_BASE_URL=https://bandao-api.ccmos.tw` as a build-time
   variable.

9. **Trigger first deploys.** Watch logs. ✓ when `/healthz` returns 200 and
   admin-web's index document loads.

10. **DNS:** create CNAME `bandao-api.ccmos.tw` → Zeabur api target,
    CNAME `bandao-admin.ccmos.tw` → Zeabur admin-web target. In Zeabur,
    attach the custom domain to each service and wait for TLS issuance.
    ✓ when `curl -vI https://bandao-api.ccmos.tw/healthz` returns 200 over
    a valid public-CA certificate.

11. **GitHub branch protection on `main`** (repo Settings → Branches):
    - Require pull request before merging
    - Require status checks: `api / fmt + clippy + test`,
      `admin-web / typecheck + test + build`, `app / test`
    - Require linear history
    - Disallow force pushes and deletions

12. **Register the first Org + admin via admin-web.** Open
    `https://bandao-admin.ccmos.tw`, follow the existing register flow,
    save credentials in the operator's password manager.

13. **Run the smoke checklist** below and only declare prod live after it
    passes end-to-end.

## Smoke checklist

Run this after every notable deploy (not every commit):

- [ ] `curl https://bandao-api.ccmos.tw/healthz` → `{"status":"ok"}`, valid TLS.
- [ ] `https://bandao-admin.ccmos.tw/` loads the SPA shell, no console errors.
- [ ] Log in with the bootstrap admin in admin-web; DevTools → Network →
      login response sets a cookie with `Secure`, `HttpOnly`, `SameSite=Lax`,
      no `Domain` attribute.
- [ ] A subsequent admin-web call (e.g. `/me`) returns 200, not 401.
- [ ] On the mobile app pointed at `https://bandao-api.ccmos.tw`, log in,
      perform a checkin; verify it appears in the admin-web checkin
      dashboard within polling latency.
- [ ] CORS rejects an unrelated origin: a credentialed `fetch` from any
      other domain to the api login endpoint is blocked by the browser.
- [ ] Restart the api service from the Zeabur dashboard; admin-web does
      not show a visible outage during rollout.

## Rollback

There is no schema migration tooling — every code rollback is purely an
image swap.

**Path A: Zeabur revision rollback (fastest).** Open the affected service →
Deployments → previous successful revision → "Roll back". Health probe
flips traffic back. Use this for hot-fixing user-visible incidents.

**Path B: `git revert` push (canonical).** From a clean clone:

```bash
git revert <bad-commit-sha>
git push origin main
```

Branch protection requires the revert PR to pass CI, then Zeabur picks up
the new `main` automatically. Use this when you want the rolled-back state
to be the next deploy and the next thing CI verifies.

Mongo state is unaffected by either path. If a prod commit corrupted data,
run a restore from S3 instead — see below.

## Tailscale auth-key rotation

1. In the Tailscale admin console, generate a new reusable, non-ephemeral
   key tagged `tag:bandao-api`.
2. Update the `TS_AUTHKEY` env var on the api service in Zeabur.
3. Redeploy the api service (or wait for the next merge to `main`).
4. The previous container picks up the new key on the next rollout; old
   nodes drop off the tailnet automatically.
5. Revoke the old key in the Tailscale admin console.

The existing image is fine — only the env var needs to change.

## Restoring Mongo from S3

Use this if data is destroyed in production and the daily dump is the
recovery source. **Practice this monthly** via the drill script — see
[`infra/mongo-host/README.md`](./infra/mongo-host/README.md).

1. Confirm which dump to restore from (`aws s3 ls s3://$S3_BUCKET/daily/`).
2. Stop writes by pausing the api Zeabur service or scaling it to zero.
3. Mount the operator's `age` private key at a tmpfs path (see drill
   instructions). Never persist the key on the Mongo host.
4. Stream the dump back into the live database:

   ```bash
   aws s3 cp - s3://$S3_BUCKET/daily/<chosen>.archive.gz.age - \
     | age -d -i /run/.../age.key \
     | mongorestore --uri="$MONGO_URI" --gzip --archive --drop
   ```

   `--drop` replaces existing collections wholesale. Use a scratch DB and
   selective re-import if you need a partial restore — drill script shows
   the namespace-rewriting flag (`--nsFrom`/`--nsTo`).

5. Restart / scale up the api service.
6. Run the smoke checklist.

## CI/CD model

The repository runs three GitHub Actions workflows on PR + push to main:
`api`, `admin-web`, `app`. They are the gate enforced by branch protection.

**Zeabur** has its own GitHub integration. On a push to `main`, Zeabur
rebuilds and rolls out whichever service's source path changed (`api/` or
`admin-web/`). There is no `deploy.yml` workflow in this repo and no
Zeabur API token to manage. The contract is: a commit on `main` that
passed required checks deploys automatically.

If a CI-failing commit ever reaches `main` (e.g. branch protection was
relaxed), Zeabur will still attempt a deploy. The mitigation is keeping
branch protection rules tight, not adding gating logic on the platform
side.

## Out-of-scope (tracked in `ROADMAP.md`)

- `/readyz` endpoint with deep dependency checks (Mongo ping, tailnet up).
- Centralized log aggregation / metrics (Loki, Sentry, Grafana).
- Staging environment.
- Multi-region or HA Mongo.
- Backup promotion from daily → weekly → monthly snapshots.
- Automated alerting from a failed restore drill.
