<script setup lang="ts">
// Public download page. No middleware applied — the route stays reachable for
// authenticated users, unauthenticated visitors, and external webview / browser
// launches alike, so an admin can share the raw URL with staff who have no
// admin account. Mirrors the public-route convention in pages/privacy.vue.
//
// iOS ships via Unlisted Distribution (unsearchable on the App Store), so this
// page is effectively the sole discovery path for the iPhone build. The App
// Store link is country-neutral (id only) on purpose — the app is Taiwan-only,
// and a region-specific /us/ path can fail to resolve.
import QRCode from 'qrcode'

definePageMeta({ layout: false })

const APP_STORE_URL = 'https://apps.apple.com/app/id6767153656'
const PLAY_STORE_URL = 'https://play.google.com/store/apps/details?id=tw.ccmos.app.bandao'
const PRIVACY_PATH = '/privacy'
const SUPPORT_EMAIL = 'support@ccmos.tw'

// Bound (not literal) so the template compiler treats these as runtime strings
// served from public/, rather than build-time asset imports to resolve.
const APP_STORE_BADGE = '/badges/app-store-badge.svg'
const PLAY_STORE_BADGE = '/badges/google-play-badge.png'

// QR codes are generated from the same link constants so a QR can never drift
// from the link it sits beside. Rendered as inline SVG (pure JS — no canvas)
// during setup, so they ship in the initial HTML with no hydration flash.
const qrOpts = { type: 'svg', margin: 1 } as const
const appStoreQr = ref(await QRCode.toString(APP_STORE_URL, qrOpts))
const playStoreQr = ref(await QRCode.toString(PLAY_STORE_URL, qrOpts))
</script>

<template>
  <main class="min-h-screen px-4 py-10">
    <div class="mx-auto max-w-2xl space-y-8">
      <header class="text-center">
        <h1 class="text-2xl font-semibold text-slate-900">
          下載 班到 App
        </h1>
        <p class="mt-1 text-sm text-slate-500">
          為小型團隊打造的多組織打卡 App。掃描 QR code 或點擊下方按鈕即可安裝。
        </p>
      </header>

      <div class="grid gap-6 sm:grid-cols-2">
        <!-- iOS -->
        <section
          class="flex flex-col items-center gap-4 rounded-xl border border-slate-200 bg-white p-6 shadow-sm"
        >
          <h2 class="text-lg font-semibold text-slate-900">
            iPhone / iPad
          </h2>
          <div
            class="h-44 w-44 rounded-md border border-slate-100 [&>svg]:h-full [&>svg]:w-full"
            role="img"
            aria-label="App Store 下載 QR code"
            data-testid="ios-qr"
            v-html="appStoreQr"
          />
          <a
            :href="APP_STORE_URL"
            target="_blank"
            rel="noopener"
            data-testid="ios-badge-link"
          >
            <img
              :src="APP_STORE_BADGE"
              alt="在 App Store 下載"
              class="h-12 w-auto"
            >
          </a>
        </section>

        <!-- Android -->
        <section
          class="flex flex-col items-center gap-4 rounded-xl border border-slate-200 bg-white p-6 shadow-sm"
        >
          <h2 class="text-lg font-semibold text-slate-900">
            Android
          </h2>
          <div
            class="h-44 w-44 rounded-md border border-slate-100 [&>svg]:h-full [&>svg]:w-full"
            role="img"
            aria-label="Google Play 下載 QR code"
            data-testid="android-qr"
            v-html="playStoreQr"
          />
          <a
            :href="PLAY_STORE_URL"
            target="_blank"
            rel="noopener"
            data-testid="android-badge-link"
          >
            <img
              :src="PLAY_STORE_BADGE"
              alt="在 Google Play 取得"
              class="h-12 w-auto"
            >
          </a>
        </section>
      </div>

      <footer class="flex flex-wrap items-center justify-center gap-x-6 gap-y-2 text-sm text-slate-500">
        <NuxtLink
          :to="PRIVACY_PATH"
          class="text-slate-600 underline underline-offset-2 hover:text-slate-900"
        >
          隱私政策
        </NuxtLink>
        <a
          :href="`mailto:${SUPPORT_EMAIL}`"
          class="text-slate-600 underline underline-offset-2 hover:text-slate-900"
        >
          {{ SUPPORT_EMAIL }}
        </a>
      </footer>
    </div>
  </main>
</template>
