# admin-web tests

Vitest + `@nuxt/test-utils` + happy-dom + `@vue/test-utils`. Tests mirror
the source tree (`pages/`, `components/`, `composables/`, `middleware/`).

```bash
pnpm test         # one-shot
pnpm test:watch   # watch mode
```

`pages/privacy.test.ts` is the canonical example for component tests —
copy and adapt when adding new tests.
