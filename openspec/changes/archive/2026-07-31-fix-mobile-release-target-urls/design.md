## Context

`Env.compileTimeDefault()` resolves the API base URL in three layers:

```
  1. runtime override (伺服器設定 → secure storage)
        │ absent ↓
  2. --dart-define=API_BASE_URL              ← baked at build time
        │ empty string ↓
  3. Platform.isAndroid ? http://10.0.2.2:9090
                        : http://localhost:9090
```

Layer 3 is correct for `flutter run`. The defect is that nothing ever checked that a *release* build got past it.

`release_ios.sh` passes `--dart-define=API_BASE_URL`, so iOS lands on layer 2. `DEPLOY.md`'s Android procedure is a bare `flutter build appbundle --release`, so Android lands on layer 3 and ships the emulator loopback alias. That procedure has been documented this way since 2026-05-07; iOS was never protected by design, only by one script's author happening to think of it.

Confirmed empirically against the two artifacts built on 2026-07-31:

| Artifact | Built via | `https://bandao-api.ccmos.tw` present |
|---|---|---|
| `bandao_app.ipa` (13:17) | `release_ios.sh` | yes |
| `app-release.aab` (13:11) | bare `flutter build appbundle` | no |

The iOS artifact is the control: it proves a dart-define leaves the value in the Dart snapshot's string pool, so the Android artifact's absence of that string is real, not an artifact of the inspection method.

Android's exposure is compounded: the app declares no `usesCleartextTraffic` and ships no `network_security_config`, so with Flutter's default `targetSdk` (≥28) the platform blocks cleartext HTTP outright. `http://10.0.2.2:9090` would fail even on a device where that address meant something.

`PRIVACY_URL` is worse in one respect and milder in another. No script or document passes it on either platform, so both artifacts link the location-consent dialog at a loopback address — but both store listings declare `https://bandao-admin.ccmos.tw/privacy` (`app/store_metadata/ios/privacy_url.txt`, `app/store_metadata/android/privacy_policy_url.txt`), so the in-app link contradicts what was declared to app review, on the exact screen review scrutinises.

`openspec/specs/mobile-release/spec.md` never mentions the API base URL or the privacy URL. The property that broke was never a requirement.

## Goals / Non-Goals

**Goals:**

- Cutting an Android release is a scripted path that bakes the production URLs, symmetric with iOS.
- A release build that failed to bake the production URLs cannot reach an upload step.
- The privacy URL in a shipped binary cannot silently diverge from the URL declared to the stores.
- `mobile-release` states the property, so the guard has something to be a guard *for*.

**Non-Goals:**

- Cutting and shipping the corrected bundle. This change makes it possible; performing it is an operator action.
- Determining whether previously published Android bundles carried the same defect. Left as an open question below — it needs an artifact only Play Console can supply.
- Removing the layer-3 dev fallback from `Env`. It is right for `flutter run`; the bug is the absent check on release builds, not the fallback's existence.
- Promotion beyond the `internal` track. `upload_android.sh` deliberately refuses to make that a scripted decision, and this change keeps that stance.
- Android cleartext policy. The app should never talk plain HTTP in release; that the platform also blocks it is a second line of defence worth keeping, not a thing to relax.

## Decisions

### D1: Verify the built artifact, not the build invocation

After the build, unpack the `.aab` / `.ipa`, search the Dart snapshot for the expected production URLs, and exit non-zero when absent.

Checking the artifact rather than the arguments is the point. A script can log the flags it believes it passed and still produce a binary without them — a typo in the define name, a stale build directory, a Flutter version that changes how defines are threaded. The artifact is the thing that gets uploaded, so the artifact is what gets asserted. This check is what surfaced the incident.

Alternatives considered:

- **Throw from `Env.compileTimeDefault()` when `kReleaseMode` and the define is empty.** Converts silent misconfiguration into an immediate crash, which is loud — but only after the binary is installed on a device. It also means a genuinely misconfigured build is a total outage rather than a broken login, and it puts a release-pipeline concern inside application code.
- **A CI check.** CI does not cut releases here; releases are cut locally by an operator. A check that does not run on the machine doing the work is not a guard.

### D2: Assert presence, never absence

The verification can only assert that the production URL **is** present. It cannot assert that `10.0.2.2` or `localhost` is **absent**.

Whether a loopback literal survives into an artifact is not predictable. The control iOS artifact contains `http://localhost:9090` *and* `https://bandao-api.ccmos.tw` together. On Android, a bundle built correctly with both defines drops `http://10.0.2.2:3000/privacy` — supplying `PRIVACY_URL` makes that fallback branch dead code, which is shaken out — while keeping `http://10.0.2.2:9090` anyway.

So an absence check does not merely produce false failures; it behaves differently per URL, red-flagging good releases on one and never firing on the other. Worth an explicit comment in the script, because it is the natural first instinct.

### D3: Grep the binary directly rather than piping through `strings`

`grep -a -q "$URL" <binary>` finds the literal without depending on `strings`, which on macOS routes through `xcrun` and emits cache-file errors under restricted environments. One less moving part in a script whose entire job is to be trustworthy.

For the `.aab`, check every `base/lib/*/libapp.so` present rather than assuming `arm64-v8a` — the bundle carries one snapshot per ABI, and a per-ABI check keeps the assertion honest if the ABI set changes.

### D4: `release_android.sh` does not bump the version

`release_ios.sh` bumps `pubspec.yaml`'s build number by default. The Android script must not, because that counter is shared: `DEPLOY.md` requires both platforms be cut from the **same** build number so one number identifies one binary pair, and a second bumping script would guarantee they diverge whenever both are cut.

So the Android script builds at whatever `pubspec.yaml` currently says, and states the version it is building in its output. Bumping stays a deliberate act performed once, by the iOS script or by hand.

### D5: The privacy URL is read from the store metadata files

`PRIVACY_URL` is sourced from `app/store_metadata/ios/privacy_url.txt` and `app/store_metadata/android/privacy_policy_url.txt` rather than hardcoded in each script.

These files are already the source of truth for what the stores are told. Reading them makes divergence between the declared policy URL and the in-app link structurally impossible instead of merely unlikely — which matters, because that divergence is exactly the defect being fixed, and a hardcoded constant in two scripts is how it would come back.

The API base URL keeps its existing `BANDAO_API_URL`-overridable default, matching `release_ios.sh`; there is no store metadata file that declares it.

### D6: `DEPLOY.md`'s Android section points at the script

The bare `flutter build appbundle --release` block is replaced by the scripted path, and the "the dart-define is required — without it login silently fails" warning that exists in the iOS section is carried over. The manual sequence stays documented as an escape hatch, but with the defines shown inline, so copying it cannot reproduce the defect.

## Risks / Trade-offs

- **The URL string could appear in the artifact for an unrelated reason**, passing verification on a binary that does not actually use it — e.g. if store metadata is ever bundled as an asset. → Low: the check reads the Dart snapshot specifically, not the whole archive. Accepting a weak false-negative here is the cost of a check that needs no code execution.

- **`--obfuscate` would not defeat the check but might look like it should.** → Dart obfuscation renames symbols; string literals survive. If obfuscation is adopted later, the check keeps working, but the next person will reasonably wonder — worth a line in the script.

- **The check hardcodes archive-internal paths** (`base/lib/*/libapp.so`, `Payload/Runner.app/Frameworks/App.framework/App`). A Flutter or bundletool layout change breaks it. → It breaks *closed*: a missing path means the assertion fails and the release stops, which is the correct direction for a guard to fail.

- **Two scripts now carry near-identical verification logic.** → Small enough to duplicate honestly. A shared helper sourced by both is the obvious refactor if a third platform appears; extracting it now would be speculative.

- **The corrected build ships as `0.4.4+14`, meaning users see a version bump for what is, in code terms, nothing.** → Unavoidable: Play rejects a reused `versionCode` and the 0.4.3 train has shipped. The release note should say what actually changed.

## Open Questions

- **Were previously published Android bundles also built without the define?** Local evidence cannot settle it: the API records no device platform (no `user_agent` / `platform` / `device` field anywhere in `api/src`), and the only `.aab` on disk is the 2026-07-31 build. Play Console → App bundle explorer → download a prior `versionCode` and run the same check. The answer decides whether rolling back to the previous release is a viable stopgap or a no-op — worth resolving before the hotfix ships, though it does not block this change.
