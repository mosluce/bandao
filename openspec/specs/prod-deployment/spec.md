# prod-deployment Specification

## Purpose
TBD - created by archiving change add-zeabur-deployment. Update Purpose after archive.
## Requirements
### Requirement: api and admin-web are deployed to Zeabur with public TLS hostnames

The production stack SHALL run on Zeabur as two services: `api` exposes `https://bandao-api.ccmos.tw` and `admin-web` exposes `https://bandao-admin.ccmos.tw`. Both hostnames SHALL serve over HTTPS with valid certificates issued by Zeabur's automated TLS pipeline. Plain HTTP requests to either hostname SHALL redirect to HTTPS or be rejected.

#### Scenario: api endpoint serves over HTTPS

- **WHEN** any client issues `GET https://bandao-api.ccmos.tw/healthz`
- **THEN** the response SHALL be `200 OK` over a valid TLS connection
- **AND** the certificate SHALL chain to a public CA

#### Scenario: admin-web is reachable

- **WHEN** a user opens `https://bandao-admin.ccmos.tw/` in a browser
- **THEN** the Nuxt SPA index document SHALL load
- **AND** the asset URLs in that document SHALL resolve to `https://bandao-admin.ccmos.tw/...`

#### Scenario: HTTP downgrade is not silently accepted

- **WHEN** any client requests either hostname over plain `http://`
- **THEN** the platform SHALL respond with an HTTPS redirect or refuse the connection
- **AND** the api SHALL NOT serve session cookies over a non-TLS connection

### Requirement: api exposes a shallow health endpoint for the platform probe

The api SHALL expose `GET /healthz` returning `200 OK` with a small JSON body (e.g. `{"status":"ok"}`) as soon as the HTTP server is bound and ready to accept connections. The endpoint SHALL NOT depend on MongoDB connectivity, Tailscale liveness, or any external service. It SHALL respond in under 100 ms under normal conditions and require no authentication.

#### Scenario: healthz returns 200 immediately after listen

- **WHEN** the api process binds its `BANDAO_LISTEN_ADDR` and is ready to serve
- **AND** any client requests `GET /healthz` (no auth header, no cookie)
- **THEN** the response SHALL be `200 OK` with a JSON body containing `"status":"ok"`

#### Scenario: healthz survives Mongo outage

- **WHEN** MongoDB is unreachable from the api container
- **AND** a client requests `GET /healthz`
- **THEN** the response SHALL still be `200 OK`
- **AND** the response SHALL NOT have attempted a Mongo round-trip

### Requirement: api connects to MongoDB only over a private Tailscale network

In production the api container SHALL join the operator's Tailscale tailnet at startup using a non-ephemeral, tagged auth key supplied via env var. The `BANDAO_MONGO_URI` SHALL resolve to the Mongo host's MagicDNS name on that tailnet. The MongoDB host SHALL NOT expose port `27017` (or its TLS variant) on any public interface; access SHALL be restricted to the tailnet interface and `localhost`.

#### Scenario: api joins the tailnet before serving traffic

- **WHEN** the api container starts in production
- **THEN** the tailscale daemon SHALL come up before the api process accepts traffic
- **AND** the api SHALL be able to resolve and reach the Mongo host's tailnet hostname

#### Scenario: Mongo is not reachable from the public internet

- **WHEN** any host outside the tailnet attempts a TCP connection to the Mongo host on port `27017`
- **THEN** the connection SHALL fail (refused, filtered, or timeout)
- **AND** no MongoDB wire protocol response SHALL be produced

### Requirement: admin-web → api requests work cross-origin with cookie credentials

When admin-web (origin `https://bandao-admin.ccmos.tw`) makes a credentialed fetch to api (origin `https://bandao-api.ccmos.tw`), the api SHALL respond with CORS headers that allow the request, including `Access-Control-Allow-Origin: https://bandao-admin.ccmos.tw` and `Access-Control-Allow-Credentials: true`. Session cookies set by the api SHALL be `Secure`, `HttpOnly`, `SameSite=Lax`, and host-only (no explicit `Domain` attribute), and SHALL be sent on subsequent admin-web fetches because the two hosts are same-site.

#### Scenario: admin-web preflight is allowed

- **WHEN** admin-web issues an `OPTIONS` preflight to api with `Origin: https://bandao-admin.ccmos.tw` and credentials
- **THEN** the api SHALL respond with `Access-Control-Allow-Origin: https://bandao-admin.ccmos.tw` (echoing the exact origin)
- **AND** with `Access-Control-Allow-Credentials: true`
- **AND** with the requested method/headers permitted

#### Scenario: cookie set by api is sent back on next admin-web fetch

- **WHEN** the api sets a session cookie in response to a successful login from admin-web
- **THEN** the cookie SHALL have `Secure`, `HttpOnly`, and `SameSite=Lax` flags
- **AND** the cookie SHALL have no `Domain` attribute (host-only on `bandao-api.ccmos.tw`)
- **WHEN** admin-web subsequently calls api with `credentials: 'include'`
- **THEN** the browser SHALL attach that cookie to the request

#### Scenario: unrelated origin is rejected

- **WHEN** any other origin (e.g. `https://attacker.example`) issues a credentialed request to api
- **THEN** the api SHALL NOT echo that origin in `Access-Control-Allow-Origin`
- **AND** the browser SHALL block the response from being read by the attacker page

### Requirement: mobile app continues to authenticate via Bearer token against the production api

The mobile app SHALL send `Authorization: Bearer <token>` to `https://bandao-api.ccmos.tw` for authenticated requests. The api SHALL accept Bearer tokens regardless of the client's origin and without requiring CORS pre-flight (mobile is not a browser). The introduction of cookie-domain or CORS configuration for admin-web SHALL NOT regress the mobile auth path.

#### Scenario: mobile authenticated request succeeds without origin or cookie

- **WHEN** the mobile app sends a request to `https://bandao-api.ccmos.tw` with `Authorization: Bearer <valid token>` and no `Origin` or `Cookie` header
- **THEN** the api SHALL authenticate the request normally
- **AND** the response SHALL NOT depend on CORS headers

### Requirement: main is auto-deployed to production after gated CI passes

The `main` branch of the repository SHALL be the only branch that triggers production deployment. GitHub branch protection SHALL require a pull request, passing status checks for the `api`, `admin-web`, and `app` workflows, and a linear history before any commit can land on `main`. Zeabur SHALL automatically rebuild and deploy whichever service's source path (`api/` or `admin-web/`) changed in the merged commit. No deploy workflow lives in the repository.

#### Scenario: CI failure blocks merge

- **WHEN** a pull request targets `main` and any of the required workflows is failing or pending
- **THEN** the merge button SHALL be disabled by branch protection
- **AND** no deployment SHALL occur

#### Scenario: clean merge triggers a deploy

- **WHEN** a pull request with all required checks green is merged to `main`
- **AND** that merge changes files under `api/`
- **THEN** Zeabur SHALL build a new image for the `api` service from the new `main` commit
- **AND** SHALL roll it out behind the health probe so the previous revision keeps serving until the new one is healthy

#### Scenario: only the changed service redeploys

- **WHEN** a merge to `main` only changes files under `admin-web/`
- **THEN** only the `admin-web` Zeabur service SHALL rebuild
- **AND** the `api` service SHALL keep serving the unchanged image

### Requirement: production secrets are supplied as environment variables on the platform

All production-only configuration (Mongo URI, Tailscale auth key, S3 credentials, cookie/CORS values, listen address) SHALL be provided to running services via environment variables on Zeabur and on the Mongo host. The repository SHALL NOT contain real secret values, real Mongo URIs, or real S3 keys. Where an example file is helpful, it SHALL be `.env.example` with placeholder values only.

#### Scenario: repo is clean of production secrets

- **WHEN** the repository's tracked files are scanned (manually or by a tool)
- **THEN** no real Tailscale auth key, S3 access/secret key, MongoDB password, or production cookie domain SHALL appear
- **AND** any committed `.env*` file SHALL contain only example placeholder values

#### Scenario: api refuses to start with insecure cookie config

- **WHEN** `BANDAO_COOKIE_SECURE=false` is set in the production deployment
- **THEN** that configuration SHALL be flagged in the deployment runbook as forbidden in production
- **AND** the smoke step SHALL verify production responses set `Secure` cookies

### Requirement: MongoDB is backed up daily to AWS S3 with periodic restore verification

The Mongo host SHALL run an automated daily job that produces a `mongodump` archive, encrypts it under an operator-controlled public key, and uploads it to a dedicated AWS S3 bucket using `S3_ACCESS_KEY_ID` and `S3_SECRET_ACCESS_KEY` env vars. Retention SHALL be at least daily for 30 days, weekly for 12 weeks, and monthly for 12 months, enforced by S3 lifecycle policy. A monthly restore drill SHALL pull the most recent dump, restore it into a scratch database, assert non-trivial document counts, and drop the scratch database. A failed drill SHALL produce an alert reachable by the operator.

#### Scenario: daily backup uploads encrypted archive

- **WHEN** the daily backup job runs
- **THEN** a `mongodump` archive SHALL be produced from the production database
- **AND** the archive SHALL be encrypted before leaving the Mongo host
- **AND** the encrypted archive SHALL appear under the day's prefix in the S3 backup bucket

#### Scenario: retention removes old daily dumps

- **WHEN** more than 30 days have elapsed since a daily dump was uploaded
- **AND** that dump is not also retained as a weekly or monthly snapshot
- **THEN** the S3 lifecycle policy SHALL delete it

#### Scenario: monthly restore drill verifies the dump is usable

- **WHEN** the monthly restore drill runs
- **THEN** the most recent encrypted dump SHALL decrypt successfully
- **AND** restore into a scratch database
- **AND** at least one collection's document count SHALL match an expected non-zero baseline
- **AND** the scratch database SHALL be dropped at the end

#### Scenario: drill failure is surfaced

- **WHEN** any step of the monthly drill fails (decrypt, restore, count assertion)
- **THEN** an alert SHALL be sent to the operator
- **AND** that alert SHALL identify which step failed and the dump's S3 key

### Requirement: production has a documented runbook for first deploy, rollback, and routine ops

The repository SHALL contain a `DEPLOY.md` (or equivalent section in `AGENTS.md`) covering: the full env var matrix per service, first-deploy bootstrap (registering the first Org / admin via the existing register flow on `bandao-admin.ccmos.tw`), platform rollback steps (Zeabur revision rollback and `git revert` flow), Tailscale auth-key rotation, restoring from S3 backup, and verifying CORS / cookie behavior after a deploy.

#### Scenario: an operator can deploy from scratch using only the runbook

- **WHEN** an operator with shell access to the Mongo host and admin access to Zeabur, GitHub, and AWS follows the runbook end-to-end
- **THEN** they SHALL produce a working production stack with the api at `https://bandao-api.ccmos.tw`, admin-web at `https://bandao-admin.ccmos.tw`, Mongo on the tailnet, daily backups landing in S3, and a registered first admin account
- **AND** SHALL not need to read source code outside of the runbook to complete the procedure

#### Scenario: rollback procedure works without state migration

- **WHEN** a deployed commit on `main` is identified as broken
- **AND** the operator follows the documented rollback path (Zeabur one-click revision rollback or `git revert` push)
- **THEN** production SHALL return to a healthy state without manual database changes

