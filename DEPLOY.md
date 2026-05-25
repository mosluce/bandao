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

## App cut release

The Flutter app at `app/` ships independently of `api/` / `admin-web/` —
its release path is **manual upload to App Store Connect + Google Play
Console** (no CI pipeline yet; that's a separate ROADMAP item once we've
cut at least one release manually).

The full task checklist lives in
`openspec/changes/app-release-prep/tasks.md` (or its archived copy after
the change ships). This section is the operator's quick-reference card.

### Pre-reqs (one-time)

- Apple Developer account active, Bundle ID `tw.ccmos.app.bandao`
  registered in App Store Connect.
- Google Play Console account active, app created with `applicationId
  tw.ccmos.app.bandao`, Play App Signing enrolled, Internal Testing track
  configured.
- Firebase project for Bandao with iOS + Android apps registered;
  `GoogleService-Info.plist` placed at `app/ios/Runner/` and
  `google-services.json` placed at `app/android/app/`.
- Mail alias `support@ccmos.tw` forwarding to whoever fields support;
  used as the public contact in store metadata.
- Android upload keystore restored from password manager:
  - `~/.bandao/keystores/bandao-upload.jks` exists
  - `app/android/key.properties` (gitignored) populated with
    `storePassword`, `keyPassword`, `keyAlias=upload`,
    `storeFile=<absolute path to .jks>`
- iOS code signing: a valid Apple distribution certificate + provisioning
  profile in the operator's keychain (Xcode handles automatic signing
  for `tw.ccmos.app.bandao` once the Apple team `SGP5JZGDM3` matches).

### Bump the version

`app/pubspec.yaml` is the single source of truth.

1. Edit `app/pubspec.yaml`'s `version: <name>+<build>` — bump build
   number monotonically (Play / TestFlight reject duplicates).
2. Append a new entry under `## App` in `CHANGELOG.md` describing the
   user-visible delta.
3. Open a PR with these changes; let CI go green; squash-merge to `main`.
4. Tag the merge commit: `git tag app-v<name> && git push --tags`. The
   tag is purely for audit — no CI hooks off it.

### Cut Android (.aab)

```bash
cd app
flutter pub get
dart run build_runner build --delete-conflicting-outputs
flutter build appbundle --release
```

The signed `.aab` lands at
`app/build/app/outputs/bundle/release/app-release.aab`.

Upload via Play Console → Internal Testing → Create new release →
upload the `.aab`. Paste the relevant
`app/store_metadata/android/changelog/<versionCode>.txt` entry into
the release notes field. Promote Internal → Closed → Production after
smoke; review can take 1–7 days for first submission.

### Cut iOS (.ipa)

The end-to-end release wrapper handles everything (recommended):

```bash
cd app
export APP_STORE_CONNECT_API_KEY_ID=ABC123XYZ4
export APP_STORE_CONNECT_API_ISSUER_ID=12345678-1234-1234-1234-123456789012
./scripts/release_ios.sh
```

That single script does:

1. Bumps `pubspec.yaml`'s build number (`+N` → `+N+1`). Apple rejects
   re-uploads of the same build number, so this auto-increment is
   load-bearing.
2. Runs `flutter build ipa --release --dart-define=API_BASE_URL=https://bandao-api.ccmos.tw`.
   The dart-define is **required** — without it the .ipa falls back to
   `Env.compileTimeDefault` (`http://localhost:9090` on iOS), which
   means the on-device build cannot reach the prod backend and login
   silently fails.
3. Uploads the signed .ipa to App Store Connect via
   `xcrun altool --upload-app` (same API as `fastlane pilot upload`).
4. Reminds you to commit the pubspec bump + tag.

Useful flags:

- `./scripts/release_ios.sh --name 0.4.0` — bump marketing version too
  (e.g. `0.3.0+5` → `0.4.0+1`).
- `./scripts/release_ios.sh --no-bump` — re-cut the same `version+build`,
  e.g. when a previous upload was rejected by Apple before processing
  (uncommon).
- `./scripts/release_ios.sh --no-upload` — build only, skip upload.

If you'd rather drive each step yourself:

```bash
cd app
flutter pub get
cd ios && pod install && cd ..
flutter build ipa --release \
  --dart-define=API_BASE_URL=https://bandao-api.ccmos.tw
./scripts/upload_ios.sh
```

Or upload via Xcode Organizer / Transporter manually if you don't want
to set up the App Store Connect API key (see operator setup below).

After upload, the build appears in App Store Connect → TestFlight →
internal testers can install immediately. Submit to App Store review
once smoke passes; first review can take 1–3 days.

### App Review submission checklist (post-upload, pre-submit)

Driven by the 2.5.4 rejection of submission `2f88a54d-2b9a-4069-b5fa-88e2ed770187`
on 2026-05-15 — keep these in place for every future build that ships
`UIBackgroundModes: location`.

- [ ] **App Privacy form**: Verify "Precise Location" lists **both**
      use cases — "App Functionality" (the AppUser viewing their own
      trajectory in "我的工作日記") **and** "Other Purposes" (org-side
      admin records). If only one is listed, edit the data type and
      re-save before submitting.
- [ ] **Demo-day seeding**: On the demo Org / demo user that the
      App Review credentials grant access to, seed at least one full
      day of location pings so the "我的軌跡" tab has a visible
      polyline. Quickest path: open the app on a real device with
      the demo creds, tap 上班, drive / walk for a few minutes,
      tap 下班. Verify in the app that `/trajectory` shows the
      polyline before submitting.
- [ ] **App Review notes / message thread**: Paste the full body of
      `app/store_metadata/ios/app_review_replies/2.5.4-<date>.md`
      (most recent file in that directory) into App Store Connect's
      "App Review Information → Notes" field, AND into the message
      thread of the rejected submission if you're resubmitting against
      the same conversation. Fill in the `<CODE> / <demo-user> /
      <demo-pass>` placeholders before pasting.

### Capture iOS screenshots

App Store requires at least one set of iPhone (6.7"+) screenshots and —
because we ship to iPad — one set of iPad 12.9" screenshots. The
`app/scripts/take_screenshots.sh` wrapper automates this end-to-end:
boots each simulator, runs an integration test that logs in with a
test account and walks through `/login → /home → /history`, and
writes the PNGs straight into `app/store_metadata/ios/screenshots/`.

```bash
cd app
./scripts/take_screenshots.sh \
  --org-code  ABC123 \
  --username  test@example.com \
  --password  yourPassword
# (or set BANDAO_TEST_ORG_CODE / _USERNAME / _PASSWORD env vars)
```

Output:

```
store_metadata/ios/screenshots/
├── iphone_6.7/{01_login,02_home,03_history}.png
└── ipad_12.9/{01_login,02_home,03_history}.png
```

A few constraints to keep in mind:

- **Cold-start required**: the test asserts you land on `/login`. If
  the simulator still has a cached session from a prior run, wipe the
  app (`Device → Erase All Content and Settings`) or use a fresh
  simulator before re-running.
- **Test account needs Org membership**: the AppUser identified by
  the credentials must already belong to an Org so the post-login
  redirect lands on the home screen instead of forcing an Org-create
  flow.
- **History page is empty unless the test account has events**: if
  you want a populated `/history` screenshot, log in manually first
  on the same simulator, do a few clock-in / clock-out cycles, then
  run the script. Or accept the empty-state screenshot.
- **Running on a release build** (`flutter drive --release`) so the
  output has no debug banner — Apple will reject screenshots that
  show the red `DEBUG` ribbon.

Once the script finishes, eyeball the PNGs before committing. They
land in repo paths that ship to the store via §6.3 metadata upload.

### App Store Connect API key (one-time operator setup)

Required for `scripts/upload_ios.sh` and for any future fastlane-style
automation. Skip this if you're sticking with manual Transporter uploads.

1. Apple Developer Portal → Users and Access → Integrations →
   App Store Connect API → Generate API Key.
2. Name: `Bandao Upload`. Access role: `App Manager` (the minimum
   level that can upload builds).
3. **Download the .p8 file immediately** — Apple lets you download it
   exactly once. The file name is `AuthKey_<KEY_ID>.p8`.
4. Move it to altool's auto-discovery path:
   ```bash
   mkdir -p ~/.appstoreconnect/private_keys
   mv ~/Downloads/AuthKey_*.p8 ~/.appstoreconnect/private_keys/
   ```
5. Save a backup in the password manager as a single item titled
   "Bandao App Store Connect API Key":
   - Binary attachment: `AuthKey_<KEY_ID>.p8`
   - Custom field: `Key ID` (≈10 alphanumeric chars)
   - Custom field: `Issuer ID` (UUID, e.g. `12345678-1234-1234-1234-123456789012`)
   - Notes: when the key was generated, role granted, what to do if lost.
6. If the key is ever lost: Apple Developer Portal → Users and Access
   → Integrations → revoke the old key + generate a new one. Then
   update Bitwarden + the local `~/.appstoreconnect/private_keys/`
   folder.

### Store-side review tips

Both stores are sensitive to:

- **Privacy nutrition / Data Safety** must match what the app actually
  does. Bandao declares: email + location + device id (linked to
  identity, app functionality), crash diagnostics + performance data
  (not linked, app functionality). No third-party sharing, no tracking.
- **Location justification**: Bandao uses When-In-Use on iOS + Foreground
  Service on Android — no Always, no ACCESS_BACKGROUND_LOCATION. In the
  submit notes, explicitly state: "tracking starts only after the user
  taps 上班; iOS displays the blue indicator while backgrounded; tap 下班
  ends tracking; we never need geofence or terminated-state location."
- **Foreground service permission** (Play): attach a screenshot of the
  「工作期間定位追蹤中」sticky notification when filling the background
  location justification form.

### Rollback

If a shipped build is bad:

- **TestFlight / Play Internal Testing**: just don't promote that build
  further; cut a hotfix patch (bump build number, fix the bug, re-cut).
- **App Store Production**: previous version stays available until the
  new one is approved; if the new one is approved-and-broken, the
  fastest path is another patch through expedited review (Apple grants
  these for genuine prod issues).
- **Play Production**: the staged-rollout slider can be paused or rolled
  back to a smaller percentage. Cut a hotfix to replace the bad build
  entirely.

In all cases there is no database migration to revert — the app talks
to the same prod api regardless of build.

## Out-of-scope (tracked in `ROADMAP.md`)

- `/readyz` endpoint with deep dependency checks (Mongo ping, tailnet up).
- Centralized log aggregation / metrics (Loki, Sentry, Grafana).
- Staging environment.
- Multi-region or HA Mongo.
- Backup promotion from daily → weekly → monthly snapshots.
- Automated alerting from a failed restore drill.
