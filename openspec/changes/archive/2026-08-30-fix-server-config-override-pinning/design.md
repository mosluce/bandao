## Context

The effective URL is resolved as `stored override ?? Env.compileTimeDefault()`, in all build modes. The screen that edits that override does two things that combine badly:

```dart
// _seed() — no override stored, so show the default…
_privacyInput.text = (savedPrivacy == null || savedPrivacy.isEmpty)
    ? Env.privacyUrlCompileTimeDefault()
    : savedPrivacy;

// _savePrivacy() — …and write whatever is in the box, unconditionally
await storage.writePrivacyUrlOverride(url);
```

So the input is indistinguishable between "you have an override of X" and "you have no override, and X is today's default". Tapping 儲存 in the second state converts it into the first, permanently, with no visible difference afterwards. The same pair exists for the API base URL in `_seed()` / `_save()`.

This was observed live during `fix-mobile-release-target-urls`: a TestFlight device showed `http://localhost:3000/privacy` in both the "current" label and the input. That turned out to be an old build rather than a stored override, but the investigation is what surfaced the mechanism — the screen genuinely cannot distinguish the two states, which is why it was a plausible explanation in the first place. A UI whose state is ambiguous to the people who wrote it will be ambiguous to users.

Why it matters more than it looks: iOS Keychain entries survive app deletion, so a pinned override outlives a reinstall. The user's remedy — 恢復官方預設 / 清除 — is only discoverable if they suspect the pinning, and nothing surfaces it.

## Goals / Non-Goals

**Goals:**

- Opening the screen and saving without editing cannot change behaviour.
- The state "no override" is visibly distinct from "override that happens to equal the default".
- 留空即使用官方預設 describes what the screen actually does.
- The privacy override is validated as strictly as the API override in release builds.

**Non-Goals:**

- Migrating away existing overrides. Silently deleting a stored value on upgrade would break the genuine self-hosted case, where an override equal to today's default may be a deliberate pin against a vendor change. See the risk below.
- Removing the privacy-URL section from release. The self-hosted story is that an operator points the app at their own deployment; their privacy policy lives on their host, so the field belongs there — it just needs the same validation as its neighbour.
- Changing the resolution order, the storage layer, or the compile-time defaults. Those are correct; only the editor is wrong.
- Surfacing "you have an override pinned to the default" as a warning. Once the input starts blank, the presence of an override is legible from the field itself.

## Decisions

### D1: Seed the inputs empty, and treat blank as "clear"

When no override is stored the input renders empty. The effective URL is already displayed directly above each field (目前實際使用的網址 / 目前隱私政策網址), so nothing is hidden by this — the value moves from a place where it is editable-and-savable to a place where it is only informative, which is what it always was.

Blank + 儲存 clears the override instead of failing validation. That makes the field's own helper text (留空即使用官方預設) true for the first time.

Alternative considered: keep the pre-fill and add a "this is the default, not an override" hint. It leaves the trap in place and asks the user to read their way out of it.

### D2: A typed value is stored as-is, even when it equals the default

The screen does not compare what the user typed against the compile-time default. If they enter a URL and save, that value is persisted as the override, whatever it happens to equal.

This follows from the semantics the product owner stated: **the default cannot be overridden; what the override sets is the current value.** The default is a fixed property of the build — always the official URL — and no user action redefines it, which is why 恢復官方預設 can always return to it. The override is a separate layer that says "use this instead, for now". A user who types the official URL into that layer is setting the current value to the official URL, which is a coherent thing to have asked for, and second-guessing it would be the screen deciding it knows better than the person typing.

Accidental pinning is fully addressed by D1: the field is empty when no override is stored, so saving without editing saves blank, which clears. There is no path from "opened the screen and tapped save" to a stored override, so a second guard would buy nothing.

Rejected: clearing the override when the typed value equals the default. It closes no remaining hole, and it produces a confusing interaction — the user types a URL, taps save, and watches the field go blank, with no explanation that the app decided their input was redundant.

### D3: Blank is handled by the caller, not by `validateBaseUrlOverride`

`validateBaseUrlOverride` keeps rejecting `""` as `malformed`. The screen checks for blank *before* validating and routes it to the clear path.

Keeping "is this a well-formed override" separate from "did the user ask to clear" means the validator stays a pure predicate over candidate URLs, and the empty string does not acquire a second meaning inside it that every future caller has to know about.

### D4: The privacy override reuses `validateBaseUrlOverride`

Rather than growing a parallel validator, `_savePrivacy()` calls the same function. Release requires `https`; debug accepts any scheme with a host.

The two URLs have the same threat model — a URL the app will open on the user's behalf, on a device where the release build should never speak cleartext — so a second set of rules is a second thing to drift. This also means the privacy field starts honouring the ATS/cleartext reasoning that the base-URL requirement already documents.

### D5: The clear-token rule is unchanged and stays keyed on the effective URL

`_save()` clears the bearer token when the new URL differs from the current *effective* base URL. That logic is correct and stays: switching servers must drop a session issued by the old one, and comparing against the effective URL handles "override → default" as well as "default → override".

This is the screen's only comparison against a URL, and it is deliberately against the effective value rather than the compile-time default. It answers "is the user changing servers?", which is a question about what the app is talking to right now — not about what the build was shipped with. The blank path from D1 must run through the same rule: clearing an active override is a server change and drops the session too.

## Risks / Trade-offs

- **Devices already carrying a default-equal override keep it.** → No migration, per the Non-Goals: on upgrade the app cannot tell an accidental pin from a deliberate one, and deleting the wrong one points a self-hoster back at the official server without asking. After this change the input renders that override's value while the field-is-empty case means "no override", so the state is at least legible; clearing it is one tap.

- **An empty input reads as "broken" to someone expecting to see the current URL.** → The effective URL sits immediately above, and the helper text explains what blank means. This is the conventional shape for an override field.

- **A user can still end up with an override equal to the default, by typing it.** → Accepted (D2). It is a deliberate act, it is visible in the field afterwards, and it costs one tap to clear. What this change removes is the *accidental* path, which was the whole defect.

- **`validateBaseUrlOverride` applied to the privacy URL rejects values that used to save in release.** → Intended: those values were `http://` URLs the app would then fail to open under ATS anyway. Anyone affected has a pinned override that this change makes visible.
