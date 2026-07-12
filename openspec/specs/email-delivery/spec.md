# email-delivery Specification

## Purpose

Defines the system's transactional-email abstraction: a swappable provider trait, its Resend-backed production implementation, and the fail-soft failure semantics every caller relies on. First consumer is password-reset (`dashboard-auth`); designed to be reused by future email-based features (member invite-by-email, register email verification).

## Requirements

### Requirement: Transactional email is sent through a swappable provider abstraction

The system SHALL send transactional email (password-reset links, and future email-based features) through an `EmailSender` trait with exactly one production implementation (Resend) and at least one non-sending implementation used when no provider is configured or in tests. The system SHALL select the implementation based on whether `RESEND_API_KEY` is configured — when absent, the system SHALL use a no-op implementation that logs the attempted send and returns success, so that no code path outside of production requires a real Resend account.

#### Scenario: Configured deployment sends via Resend

- **WHEN** `RESEND_API_KEY` is set and the system sends an email
- **THEN** the email is submitted to Resend's API using the configured API key and from-address

#### Scenario: Unconfigured deployment no-ops without failing the caller

- **WHEN** `RESEND_API_KEY` is not set and the system attempts to send an email
- **THEN** the send is logged and treated as successful
- **AND** no network request to any email provider is made

### Requirement: Email send failures are fail-soft and never surfaced to the end user as a distinguishable error

The system SHALL treat every email-send failure (provider error, network failure, timeout) as non-fatal to the caller's request — the failure SHALL be logged with enough detail to diagnose the cause, but SHALL NOT change the HTTP response returned to the end user for the operation that triggered the send.

#### Scenario: A provider error does not change the caller's response

- **WHEN** sending an email fails for any reason during a request that triggers a send
- **THEN** the failure is logged
- **AND** the HTTP response to the caller is unaffected by the failure
