import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'
import { mountSuspended } from '@nuxt/test-utils/runtime'

import DownloadPage from '~/pages/download.vue'

const APP_STORE_URL = 'https://apps.apple.com/app/id6767153656'
const PLAY_STORE_URL = 'https://play.google.com/store/apps/details?id=tw.ccmos.app.bandao'

describe('DownloadPage', () => {
  it('renders without error', async () => {
    const wrapper = await mountSuspended(DownloadPage)
    expect(wrapper.exists()).toBe(true)
  })

  it('links the App Store badge to the country-neutral iOS URL', async () => {
    const wrapper = await mountSuspended(DownloadPage)
    const link = wrapper.get('[data-testid="ios-badge-link"]')
    expect(link.attributes('href')).toBe(APP_STORE_URL)
  })

  it('links the Google Play badge to the public Android listing', async () => {
    const wrapper = await mountSuspended(DownloadPage)
    const link = wrapper.get('[data-testid="android-badge-link"]')
    expect(link.attributes('href')).toBe(PLAY_STORE_URL)
  })

  it('renders an inline-SVG QR code for each store', async () => {
    const wrapper = await mountSuspended(DownloadPage)
    const iosQr = wrapper.get('[data-testid="ios-qr"]')
    const androidQr = wrapper.get('[data-testid="android-qr"]')
    expect(iosQr.html()).toContain('<svg')
    expect(androidQr.html()).toContain('<svg')
  })

  it('serves both store badges from local public assets, not hotlinked', async () => {
    const wrapper = await mountSuspended(DownloadPage)
    const html = wrapper.html()
    expect(html).toContain('/badges/app-store-badge.svg')
    expect(html).toContain('/badges/google-play-badge.png')
  })

  it('renders the privacy link and support email', async () => {
    const wrapper = await mountSuspended(DownloadPage)
    const html = wrapper.html()
    expect(html).toContain('/privacy')
    expect(wrapper.text()).toContain('support@ccmos.tw')
  })

  it('does not register any route middleware', () => {
    // Static-source check — the page must stay public (no auth redirect) so an
    // admin can share the URL with staff who have no account.
    const source = readFileSync(resolve(process.cwd(), 'pages/download.vue'), 'utf8')
    expect(source).not.toMatch(/middleware\s*:/)
  })
})
