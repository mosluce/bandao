## 1. Dev dependencies

- [x] 1.1 Add to `admin-web/package.json` `devDependencies`: `vitest`, `@nuxt/test-utils`, `happy-dom`, `@vue/test-utils`. Use latest stable for each. (`vitest` ~3.x, `@nuxt/test-utils` whatever supports Nuxt 3.21.x; `happy-dom` ~15.x; `@vue/test-utils` ~2.x.)
- [x] 1.2 Run `pnpm install` from `admin-web/`. Verify lockfile updates cleanly and there's no unmet peer dep warning.
- [x] 1.3 Sanity-check that `pnpm typecheck` and `pnpm build` still pass after the new deps land.

## 2. Vitest config

- [x] 2.1 Create `admin-web/vitest.config.ts`:
  ```ts
  import { defineVitestConfig } from '@nuxt/test-utils/config'

  export default defineVitestConfig({
    test: {
      environment: 'nuxt',
      environmentOptions: {
        nuxt: {
          domEnvironment: 'happy-dom',
        },
      },
    },
  })
  ```
- [x] 2.2 Add `tsconfig.json` reference for the test files if needed (Nuxt's auto-prepare may already cover it — verify after install). If `pnpm typecheck` complains about missing types in `test/`, add `"include"` for it.

## 3. package.json scripts

- [x] 3.1 Add `"test": "vitest run"` to `admin-web/package.json` scripts.
- [x] 3.2 Add `"test:watch": "vitest"` for local development (default vitest mode is watch).

## 4. Test directory scaffold

- [x] 4.1 Create `admin-web/test/` directory.
- [x] 4.2 Create `admin-web/test/pages/` subdirectory.
- [x] 4.3 (Optional but recommended) Add a `admin-web/test/README.md` one-paragraph orientation: where tests live, how to run, the convention to mirror source structure. Keep it short — < 15 lines.

## 5. privacy.vue test (履行 add-org-privacy-policy §3 deferred)

- [x] 5.1 Create `admin-web/test/pages/privacy.test.ts`. Use `mountSuspended` from `@nuxt/test-utils/runtime`.
- [x] 5.2 Test 1 — page mounts without throwing. Use a `describe('PrivacyPage', () => {...})` outer block, `it('renders without error', ...)`.
- [x] 5.3 Test 2 — all 9 section headings present. Loop the expected heading texts (`'1. 適用範圍'`, `'2. 我們蒐集的資料'`, ..., `'9. 政策更新'`) and assert each appears in `wrapper.text()`.
- [x] 5.4 Test 3 — disclaimer renders. Assert `wrapper.text()` contains `'本政策範本未經法律審查'`.
- [x] 5.5 Test 4 — placeholder email renders. Assert `wrapper.text()` contains `'noreply@example.com'`.
- [x] 5.6 Test 5 — no middleware registered. Read `admin-web/pages/privacy.vue` source via `fs.readFileSync` (test runtime, not at compile time), assert it does NOT contain a `middleware:` key inside any `definePageMeta(...)` call. The simplest assertion: `expect(source).not.toMatch(/middleware\s*:/)`.

## 6. CI integration

- [x] 6.1 Edit `.github/workflows/admin-web.yml` — between the existing `pnpm typecheck` and `pnpm build` steps, add `- run: pnpm test`.
- [x] 6.2 Sanity-check the workflow YAML is still valid (e.g. `gh workflow view admin-web` if available, or just visual review of indentation).

## 7. Verification

- [x] 7.1 Run `pnpm test` locally — all 5 privacy.vue assertions pass.
- [x] 7.2 Run `pnpm typecheck` — no new errors.
- [x] 7.3 Run `pnpm build` — production build still succeeds.
- [x] 7.4 Make a deliberately broken assertion (e.g. expect `'10. 政策更新'`) to verify failure mode is informative; revert.

## 8. Documentation

- [x] 8.1 Append a "Testing" section to `admin-web/README.md` covering: framework choice (vitest + @nuxt/test-utils), how to run (`pnpm test` / `pnpm test:watch`), where tests live (`admin-web/test/` mirror source tree), and reference to `pages/privacy.test.ts` as a starting template. Keep it tight — one short paragraph + the two commands.

## 9. Smoke (CI)

- [x] 9.1 After archive auto-commit pushes, check that the `admin-web` workflow on GitHub Actions runs typecheck → test → build, all green.
