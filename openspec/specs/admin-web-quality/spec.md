# admin-web-quality Specification

## Purpose
TBD - created by archiving change add-admin-web-test-infra. Update Purpose after archive.
## Requirements
### Requirement: admin-web ships with a working vitest + Nuxt test environment

The `admin-web/` codebase SHALL include a configured `vitest` test runner with `@nuxt/test-utils` (Nuxt-aware environment + `mountSuspended` helper), `happy-dom` (DOM emulation), and `@vue/test-utils` (mount + interaction). Tests SHALL live under `admin-web/test/` mirroring the source tree (`pages/`, `components/`, `composables/`, `middleware/`). The package SHALL expose `pnpm test` (one-shot) and `pnpm test:watch` (watch mode) scripts.

#### Scenario: Test framework is invocable

- **WHEN** a developer runs `pnpm test` from `admin-web/`
- **THEN** vitest discovers tests under `admin-web/test/**/*.test.ts`
- **AND** the run completes with a non-error exit code when all tests pass

#### Scenario: Nuxt-aware mount is available

- **WHEN** a test imports `mountSuspended` from `@nuxt/test-utils/runtime`
- **AND** uses `defineVitestConfig` from `@nuxt/test-utils/config` with `environment: 'nuxt'` and `domEnvironment: 'happy-dom'`
- **THEN** Vue components that depend on Nuxt auto-imports / runtime config / composables can be mounted in tests

### Requirement: CI runs the admin-web test suite on every push and PR

The `admin-web` GitHub Actions workflow SHALL execute `pnpm test` after `pnpm typecheck` and before `pnpm build`. Test failures SHALL fail the workflow before the build step runs.

#### Scenario: Failing test fails CI before build

- **WHEN** a commit is pushed that introduces a failing test in `admin-web/test/`
- **THEN** the `admin-web` workflow fails at the test step
- **AND** the build step does not run

#### Scenario: Passing tests proceed to build

- **WHEN** a commit pushes with all admin-web tests passing
- **THEN** the workflow runs typecheck, then test, then build, all green

### Requirement: privacy.vue page test exists and validates structural promises

The `admin-web/test/pages/privacy.test.ts` test SHALL verify that the `/privacy` page renders without errors, contains all nine section headings (1. 適用範圍 through 9. 政策更新), surfaces the disclaimer footer (`本政策範本未經法律審查...`), and includes the platform contact placeholder (`noreply@example.com`). The test SHALL also verify that `pages/privacy.vue` does NOT register any route middleware so the page stays publicly reachable.

#### Scenario: All nine sections render

- **WHEN** the privacy page is mounted in a test
- **THEN** the rendered HTML contains the heading text for each of the 9 sections defined by `org-privacy-policy`

#### Scenario: Disclaimer and placeholder email render

- **WHEN** the privacy page is mounted in a test
- **THEN** the disclaimer text `本政策範本未經法律審查` is in the output
- **AND** the placeholder email `noreply@example.com` is in the output

#### Scenario: privacy.vue applies no middleware

- **WHEN** the source file `pages/privacy.vue` is parsed
- **THEN** it does not register a `middleware` key via `definePageMeta` (neither `auth` nor `guest`)
