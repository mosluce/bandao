# ci-pipeline Specification

## Purpose

Define the monorepo CI contract: how the three per-component GitHub Actions
workflows (`api`, `admin-web`, `app`) gate merges to `main`. Each component
exposes one stably-named required status check that always reports on a PR but
only runs its build/lint/test when that component actually changed, and the
merge policy that governs how those checks interact with branch protection.

## Requirements

### Requirement: Per-component required checks report on every PR but build only changed components

The CI system SHALL define one GitHub Actions workflow per component (`api`, `admin-web`, `app`), each exposing exactly one status check with a stable name (`fmt + clippy + test`, `typecheck + test + build`, `analyze + test` respectively). On `pull_request`, each workflow SHALL trigger unconditionally (no `paths:` filter) so its check always reports and never blocks branch protection with a permanently-pending status. Within the job, the workflow SHALL detect whether its component's paths changed and SHALL execute the heavy verification steps (toolchain setup, caching, build, lint, test) only when they did; when they did not, the job SHALL still conclude `success` so the required check reports green without building the component. On `push` to `main`, each workflow SHALL retain a `paths:` filter so it runs only when its component changed.

#### Scenario: PR touching only one component builds only that component

- **WHEN** a pull request changes files only under `app/`
- **THEN** the `analyze + test` check runs its full verification steps
- **AND** the `fmt + clippy + test` and `typecheck + test + build` checks report `success` without running their build/lint/test steps
- **AND** all three required checks are green so the PR is mergeable

#### Scenario: PR touching multiple components builds each of them

- **WHEN** a pull request changes files under both `api/` and `admin-web/`
- **THEN** the `fmt + clippy + test` and `typecheck + test + build` checks run their full steps
- **AND** the `app` check (`analyze + test`) reports `success` without running its steps

#### Scenario: A skipped component never leaves a required check pending

- **WHEN** a pull request does not touch a given component
- **THEN** that component's required status check still reports a terminal `success` conclusion
- **AND** the pull request is not blocked waiting on a check that never runs

#### Scenario: A skipped component is announced for reviewers

- **WHEN** a component's heavy steps are skipped because it was not changed
- **THEN** the job emits a `::notice::` naming the skipped component
- **AND** a reviewer can tell which components were actually verified without reading the full logs

### Requirement: Change detection uses only git, against the PR merge base

The CI system SHALL determine which components changed using only git (no third-party actions). The checkout SHALL fetch enough history to compute the diff (`fetch-depth: 0`). For `pull_request` events, detection SHALL compare the pull request base commit against the head using a merge-base diff (`git diff --name-only <base.sha>...HEAD`) and match each component's path glob (including its own workflow file). For `push` events, detection SHALL treat the component as changed (the `push` `paths:` filter already gates the trigger). The detection step SHALL evaluate paths relative to the repository root regardless of any job-level working-directory default.

#### Scenario: Detection runs without third-party actions

- **WHEN** any workflow evaluates whether its component changed
- **THEN** it uses only `git` and shell built-ins
- **AND** it does not depend on any third-party marketplace action for change detection

#### Scenario: Base-branch commits do not count as PR changes

- **WHEN** the base branch has advanced with commits unrelated to the pull request
- **THEN** the merge-base diff (`<base.sha>...HEAD`) reflects only the pull request's own changes
- **AND** those unrelated base commits do not cause a component to be treated as changed

### Requirement: Merging does not require branches to be up to date

The `main` branch protection SHALL NOT enable the "require branches to be up to date before merging" option (`strict = false`), while continuing to require all three per-component status checks to pass. A pull request whose required checks have passed SHALL remain mergeable after the base branch advances, without being forced to update its branch and re-run CI, provided no textual merge conflict exists.

#### Scenario: An unrelated merge does not force a re-run

- **WHEN** one pull request merges and advances `main`
- **AND** a second open pull request already has all required checks passing and no merge conflict
- **THEN** the second pull request remains mergeable without updating its branch or re-running CI

#### Scenario: Required checks still gate merges

- **WHEN** a pull request has a failing or missing required per-component check
- **THEN** the pull request is not mergeable until that check reports success
