# App Review reply — Guideline 2.5.4 + 3.2 → pivot to Unlisted Distribution (2026-05-26)

- **Submission ID:** 2f88a54d-2b9a-4069-b5fa-88e2ed770187
- **App / Version:** 班到 (Bandao) 0.3.1 (9)
- **Guidelines cited:** 2.5.4 (background location beyond employee tracking) **and** 3.2 (Business — public distribution for a business/org app)
- **Review device / date:** iPad Air 11-inch (M3), May 26, 2026
- **Reply date:** _<fill on send>_
- **Decision:** We accept Apple's read. 班到 is a limited-audience business app; we are moving it to
  **Unlisted App Distribution** rather than public App Store. See operator checklist below.

---

## Why this round is different from round 1

Round 1 (see `2.5.4-2026-05-15-r1.md`) tried to satisfy 2.5.4 by adding an end-user-facing
"我的工作日記 / My Work Day" trajectory feature so the AppUser — not just the employer — benefits
from background location. App Review did not accept that framing (showing an employee their own
tracking data is still employee tracking) **and** added 3.2, concluding the app is intended for a
specific business audience and therefore does not belong on the public App Store.

Both guidelines point the same way: **the correct fix is the distribution channel, not another feature.**
Apple's own unlisted-distribution guidance lists "employee resources" and "limited audiences (part-time
employees, franchisees, partners, business affiliates)" as good candidates — which is exactly what 班到 is.
Moving to unlisted distribution resolves 3.2 and makes the background-location use (recording an
employee's on-shift trajectory for their organization) legitimate for a limited business audience.

The "我的工作日記" feature stays in the build — it is genuinely useful — but it is no longer our
argument for the entitlement.

---

## Reply body (paste into App Store Connect thread)

Hello,

Thank you for the additional review. After considering both points, we agree with your assessment:
班到 (Bandao) is a business app for a limited audience (the employees of the organizations that
subscribe to it), not a general-public app. We would like to correct our distribution choice rather
than contest it.

**On Guideline 3.2 — answers to your questions**

1. **Is the app restricted to users who are part of a single company or organization?**
   It is restricted to users affiliated with an organization. It is not a single company — 班到 is a
   multi-tenant clock-in / attendance service, so users belong to whichever organization subscribes.
   A user cannot use the app without an organization account.

2. **Is the app designed for a limited or specific group of companies?**
   It is designed for organizations that subscribe to the 班到 service and their employees. Any
   organization can become a subscriber, but the app is only usable by users an organization has
   provisioned — there is no general-public usage path.

3. **What features are intended for use by the general public?**
   None. Every screen requires authentication with an organization code and a per-user account created
   by that organization's administrator. There is no public sign-up, browsing, or content.

4. **How do users obtain an account?**
   An organization's administrator creates the organization in our admin console and issues an
   organization code plus per-employee credentials. Employees log in to the app with that code and
   their credentials. There is no self-service consumer registration inside the app.

5. **Is there any paid content in the app, and who pays?**
   There is no in-app purchase and end users (employees) pay nothing. The organization pays a
   business subscription for the service outside the app.  <!-- OPERATOR: confirm exact billing model before sending -->

**On Guideline 2.5.4 — background location**

The persistent background location entitlement is used to record an employee's on-shift movement
(trajectory / distance) for the subscribing organization. We understand this is not appropriate for
public App Store distribution. Given the app's limited business audience, we are moving 班到 to
**Unlisted App Distribution**, which Apple lists as an appropriate channel for employee/organization
apps of this kind.

**Next steps we are taking**

- We have added a note to the build's Review Notes indicating this app is intended for unlisted
  distribution.
- We have submitted a request for Unlisted App Distribution for this app.

Please let us know if you need anything else from us to proceed on the unlisted track. We are happy to
record a short screen capture of the login-gated experience if useful.

Thank you,

The Bandao team
support@ccmos.tw

---

## Unlisted App Distribution — request checklist (Apple's documented process)

1. **App must be submitted to App Review and approved / ready for distribution.** Unlisted requests
   are declined if the app has not been submitted to App Review or is in a beta/prerelease state.
   → Keep build 0.3.1 (9) (or the next build) submitted for standard review.
2. **Add a Review Note** on the submission: `This app is intended for unlisted distribution.`
3. **Submit the unlisted request form:** https://developer.apple.com/contact/request/unlisted-app/
   (Apple support page: https://developer.apple.com/support/unlisted-app-distribution/)
   - **Only the Apple Developer account Owner can open this form** — anyone else is redirected to `/contact/`.
   - The form asks for the app's **Apple ID number** (App Store Connect → App Information → Apple ID).
   - Apple's eligibility verification can take **more than 24h**.
4. Unlisted apps still go through the **full standard App Review** — the app must still pass the
   guidelines; it simply won't be discoverable in search / charts / categories and is reachable only
   via the direct App Store link Apple issues on approval.
5. On approval, distribute the unlisted link to subscribing organizations (do NOT rely on public
   search). Consider an in-app authorization gate — anyone with the link can download, so the
   org-code + credential login is our access control (already in place).

---

## Unlisted request form — free-text answers (as submitted)

**Org type:** Business · **Distribution:** Externally (to customer organizations)
**"Submitted to App Store for review?":** Yes (build 0.3.1 (9))

**Q1 — Business problem + why unlisted helps (with example):**

> 班到 (Bandao) is a multi-tenant workforce clock-in and attendance service. Organizations such as
> cleaning companies, security firms, and facility-management contractors use it so their field and
> shift employees can clock in/out and so the organization can record each employee's on-shift
> location trajectory to verify attendance and service coverage at client sites. Every user must
> authenticate with an organization code plus a per-employee account created by that organization's
> administrator — there is no public sign-up and no public-facing content, so the app has no
> general-public audience. Unlisted distribution lets each subscribing organization's employees
> install the app from a single App Store link we provide, while keeping it out of public search and
> charts where general consumers — who could never actually use it — would otherwise find it.
> Example: a cleaning company subscribes, its admin creates the org code and 30 employee accounts,
> and we send that organization the unlisted link so those 30 employees can install the app and
> clock in at client buildings.

**Q2 — Why unlisted over public App Store:**

> The app is usable only by employees of subscribing organizations — every screen is gated behind an
> organization code and provisioned credentials, with no features, content, or sign-up for the
> general public. Public listing would surface it to consumers who can never use it, produce confused
> downloads and reviews, and — as App Review noted under Guideline 3.2 — is not the appropriate
> channel for a business/organization app. The app also uses persistent background location solely to
> record employees' on-shift trajectories for their organization, which is appropriate for a limited
> business audience but not for the public App Store. Unlisted distribution matches the app's real,
> limited audience.

**Q3 — Why unlisted over private distribution via Apple Business / School Manager:**

> 班到 is a self-service multi-tenant SaaS: any organization can subscribe, and new organizations
> onboard continuously with no manual per-organization deployment step on our side. Custom App
> distribution via Apple Business Manager would require each subscribing organization to be enrolled
> in Apple Business Manager and to receive a separate managed/redemption distribution from us per
> organization. Many of our customers are small service businesses (cleaning, security, facilities)
> that are not enrolled in Apple Business Manager and run no MDM, so requiring ABM would block most of
> them from ever using the app. Unlisted distribution lets us give every subscribing organization the
> same single App Store link; their employees install it on their own devices and gain access only
> after logging in with their organization's code and credentials. This scales to many organizations
> with zero per-organization Apple Business Manager setup, while our in-app org-code + credential
> login remains the access control that prevents unauthorized use of the link.

---

## Demo credentials (fill / verify before submitting)

```
Org code:   demo
Username:   demo
Password:   demodemo

Seeding:    Seed at least 1 demo day of location pings for the demo user so the
            trajectory tab shows a visible polyline. See DEPLOY.md "iOS cut"
            → "App Review demo seeding".
```

---

## Internal notes (not sent to App Review)

- This file is the source of truth for round-2 reply text. Operator copy-pastes the "Reply body"
  section into the App Store Connect message thread, submits the unlisted request form, and adds the
  Review Note.
- Round-1 reply archived at `2.5.4-2026-05-15-r1.md`.
- Open question for operator before sending: confirm the answer to 3.2 Q5 (billing model) matches
  reality (subscription billed to the organization, no end-user IAP).
- No app code change is required for the pivot. The `UIBackgroundModes: location` entitlement and the
  trajectory feature remain; the distribution channel is what changes.
- If App Review still raises 2.5.4 after the unlisted request, escalate via the thread citing Apple's
  unlisted-distribution guidance that explicitly names employee/limited-audience apps as candidates.
