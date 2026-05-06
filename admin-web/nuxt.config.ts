export default defineNuxtConfig({
  compatibilityDate: '2025-01-01',
  // SPA-only: the API is cookie-based (HttpOnly + SameSite=Lax) and lives on a
  // different origin in dev. Keeping Nuxt as a pure client-side app avoids the
  // SSR cookie-forwarding dance for MVP.
  ssr: false,
  // DevTools 1.7 stalls dev SSR ~10s/request on this project; turn it on
  // ad-hoc when you need it.
  devtools: { enabled: false },
  modules: ['@nuxtjs/tailwindcss'],
  css: ['~/assets/css/main.css'],
  typescript: {
    strict: true,
    typeCheck: false,
  },
  runtimeConfig: {
    public: {
      apiBaseUrl: 'http://localhost:8080',
    },
  },
  app: {
    head: {
      title: '班到 admin',
      meta: [
        { charset: 'utf-8' },
        { name: 'viewport', content: 'width=device-width, initial-scale=1' },
      ],
    },
  },
})
