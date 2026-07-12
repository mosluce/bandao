## REMOVED Requirements

### Requirement: Admin can rotate the Org code

**Reason**: Evaluated as having no real-world usage — the feature existed as a defense against a leaked/exfiltrated org code, but that defense is being replaced by a login-attempt lockout mechanism (tracked in ROADMAP.md) that makes a leaked code alone insufficient for abuse, rather than by an in-band rotation lever most orgs never touched. Removing this reduces the "管理員工具" surface being reworked in `add-admin-web-sidemenu`.

**Migration**: None needed — `org.code` itself, its generation on Org creation, and its use in join/App-login resolution are all unaffected. Only the ability to replace an existing code with a new one is removed. Orgs that never rotated are unaffected; any Org relying on rotation as their only mitigation for a leaked code now has no in-band recovery path short of recreating the Org — this is the accepted trade-off (see the change's proposal for the reasoning).
