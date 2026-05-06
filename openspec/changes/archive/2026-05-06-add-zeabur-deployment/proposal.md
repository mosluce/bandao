## Why

The api and admin-web only run locally today. Mobile beta and admin onboarding need a public, always-on production environment with predictable URLs, TLS, and a real database, plus a CI/CD path so changes ship without manual sshing into a box.

## What Changes

- Stand up a production environment on **Zeabur** running two services: `api` (Rust binary in a Docker image) and `admin-web` (Nuxt SPA static build).
- Provision a self-hosted **MongoDB** on a separate operator-controlled machine, reachable from Zeabur over a **Tailscale** private network only — Mongo never exposes 27017 to the public internet.
- Allocate two public hostnames under `ccmos.tw`: `bandao-api.ccmos.tw` (api) and `bandao-admin.ccmos.tw` (admin-web). Both are subdomains of the same eTLD+1 (`ccmos.tw`), keeping admin-web ↔ api requests **same-site** so cookie auth works with `SameSite=Lax`.
- Configure CORS on the api to allow `https://bandao-admin.ccmos.tw` with credentials. Cookies set by the api are host-only (`bandao-api.ccmos.tw`), `Secure`, `HttpOnly`, `SameSite=Lax`.
- Mobile app keeps Bearer-token auth against the same api host; tokens unaffected by cookie / CORS changes.
- Add a Dockerfile for the api (multi-stage Rust → debian-slim runtime). admin-web uses Zeabur's built-in Nuxt deploy or a thin static-host image; either way it ships only `.output/public` artifacts.
- Daily `mongodump` cron on the Mongo host uploads encrypted dumps to **AWS S3** using `S3_ACCESS_KEY_ID` / `S3_SECRET_ACCESS_KEY` env vars; retention is daily×30 / weekly×12 / monthly×12, with a monthly restore drill into a scratch database.
- Adopt **CI/CD path 1**: existing GitHub Actions workflows (`api`, `admin-web`, `app`) keep running on PR and push; `main` becomes a protected branch that requires those checks plus PR review; Zeabur's GitHub integration auto-builds and auto-deploys on every `main` push. No deploy workflow lives in this repo.
- Add a `/healthz` endpoint to the api (cheap, no DB hit) so Zeabur's health probe drives zero-downtime deploys.
- Document the operational runbook (env vars, first-deploy bootstrap, backup verification, rollback) in `AGENTS.md` or a new `DEPLOY.md`.

## Capabilities

### New Capabilities

- `prod-deployment`: production hosting, networking, DNS/TLS, CI/CD gating, secrets, and backup/restore behavior for the api + admin-web stack.

### Modified Capabilities

<!-- None. Existing app-checkin / dashboard-auth / etc. specs describe behavior that is already platform-agnostic; this change adds operational requirements without altering them. -->

## Impact

- New code: `api/Dockerfile`, optional `admin-web/Dockerfile`, api `/healthz` handler, possibly `.dockerignore` files.
- New ops artifacts: Mongo host setup notes, Tailscale auth-key handling, `mongodump` cron + S3 upload script, restore-drill script.
- New config surface in api: re-uses existing `BANDAO_*` env vars; production values set on Zeabur. No new env vars in code beyond what `config.rs` already reads.
- DNS: two new CNAME records on `ccmos.tw` pointing at Zeabur targets.
- CI: no workflow changes required for tests; GitHub branch-protection rules updated outside the repo (admin task).
- External dependencies: Tailscale account, AWS S3 bucket + IAM user, Zeabur project.
- Cost: Zeabur usage (small), Tailscale free tier, S3 storage (low, dumps are gzipped + small DB), Mongo host whatever the operator already pays for.
- Risk: Mongo host availability becomes the production SPOF; mitigation is the daily backup + documented restore. Zeabur build cache for Rust is unverified — first cold deploys may take 10+ minutes.
