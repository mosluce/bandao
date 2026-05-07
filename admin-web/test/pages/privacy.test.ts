import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'
import { mountSuspended } from '@nuxt/test-utils/runtime'

import PrivacyPage from '~/pages/privacy.vue'

describe('PrivacyPage', () => {
  it('renders without error', async () => {
    const wrapper = await mountSuspended(PrivacyPage)
    expect(wrapper.exists()).toBe(true)
  })

  it('shows all nine section headings', async () => {
    const wrapper = await mountSuspended(PrivacyPage)
    const text = wrapper.text()
    const sections = [
      '1. 適用範圍',
      '2. 我們蒐集的資料',
      '3. 蒐集目的',
      '4. 保留期間',
      '5. 誰能存取您的資料',
      '6. 您的權利',
      '7. Cookie 與 Session',
      '8. 聯絡方式',
      '9. 政策更新',
    ]
    for (const heading of sections) {
      expect(text).toContain(heading)
    }
  })

  it('renders the disclaimer footer', async () => {
    const wrapper = await mountSuspended(PrivacyPage)
    expect(wrapper.text()).toContain('本政策範本未經法律審查')
  })

  it('renders the platform contact email', async () => {
    const wrapper = await mountSuspended(PrivacyPage)
    expect(wrapper.text()).toContain('support@ccmos.tw')
  })

  it('does not register any route middleware', () => {
    // Static-source check — Nuxt route middleware is opted in via
    // `definePageMeta({ middleware: ... })`. A plain text scan is enough
    // since the page has no other reason to mention `middleware`.
    // process.cwd() is the admin-web root when vitest runs.
    const source = readFileSync(resolve(process.cwd(), 'pages/privacy.vue'), 'utf8')
    expect(source).not.toMatch(/middleware\s*:/)
  })
})
