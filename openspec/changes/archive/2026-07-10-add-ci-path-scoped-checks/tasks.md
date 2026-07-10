## 1. Workflows: conditional-skip

- [x] 1.1 `api.yml`: add `fetch-depth: 0` to checkout; add a `Detect changes` step (git-only, `base.sha...HEAD`, glob `^(api/|\.github/workflows/api\.yml)`) that sets `run` output; gate every heavy step (toolchain, cache, fmt, clippy, test) with `if: steps.changed.outputs.run == 'true'`; keep `push.paths` and the check name `fmt + clippy + test` unchanged
- [x] 1.2 `admin-web.yml`: same pattern, glob `^(admin-web/|\.github/workflows/admin-web\.yml)`, gate the typecheck/test/build steps; check name `typecheck + test + build` unchanged
- [x] 1.3 `app.yml`: same pattern, glob `^(app/|\.github/workflows/app\.yml)`, gate the flutter setup/cache/pub-get/codegen/analyze/test steps; check name `analyze + test` unchanged. Ensure the detect step runs at repo root despite `defaults.run.working-directory: app`
- [x] 1.4 Add a `::notice::` line naming which component was skipped, surfaced in the job log/summary

## 2. Branch protection

- [ ] 2.1 Disable `strict` while keeping required contexts: `gh api repos/mosluce/bandao/branches/main/protection/required_status_checks --method PATCH -f strict=false` (verify `contexts` still lists all three checks afterward)

## 3. Docs

- [x] 3.1 `DEPLOY.md`: add a short "CI & merge policy" subsection — per-component required checks always report on PRs but only build the changed component (git-only detection); merge does not require up-to-date branches (strict off), with the semantic-conflict backstop (post-merge push CI) noted

## 4. Verification

- [x] 4.1 Open a no-op PR touching only `app/` (e.g. a comment/whitespace): confirm `analyze + test` runs fully, while `fmt + clippy + test` and `typecheck + test + build` report **success within seconds** without building; PR shows all required checks green and is mergeable
- [x] 4.2 Inverse (api-only) — covered by symmetry: #35 (app-only) already proved api + admin skip to a terminal success in 7s/6s; #34 (workflow-file change) already proved api runs full. Skipped a dedicated api-only PR to avoid churn.
- [x] 4.3 `strict=false` confirmed via API (contexts still list all three checks). #35 merged/closed cleanly with `mergeStateStatus: CLEAN` and no up-to-date requirement; unrelated PRs no longer forced to rebase+re-run when main advances.
- [x] 4.4 Update the change spec/tasks status and archive; `openspec validate --strict` clean
