import { describe, expect, it, vi } from 'vitest'
import { mountSuspended } from '@nuxt/test-utils/runtime'

import TrajectoryPage from '~/pages/checkin/[appUserId]/trajectory.vue'

// Mock Leaflet — we don't run a real map in tests; we only assert the
// component-level state (empty / has-data flag → DOM container presence).
// This keeps the test framework agnostic of happy-dom's DOM coverage gaps
// (createPane / getBoundingClientRect quirks).
vi.mock('leaflet', () => ({
  default: {
    map: vi.fn(() => ({
      remove: vi.fn(),
      eachLayer: vi.fn(),
      removeLayer: vi.fn(),
      fitBounds: vi.fn(),
    })),
    tileLayer: vi.fn(() => ({ addTo: vi.fn() })),
    polyline: vi.fn(() => ({ addTo: vi.fn() })),
    circleMarker: vi.fn(() => ({
      bindPopup: vi.fn(() => ({ addTo: vi.fn() })),
    })),
    latLngBounds: vi.fn(),
    TileLayer: class {},
  },
}))
vi.mock('leaflet/dist/leaflet.css', () => ({}))

// Mock Nuxt auto-import composables. Trajectory page calls useAuth /
// useCheckin / useLocationPings / useRoute / useRouter / useRuntimeConfig.
const fakeAuth = {
  currentOrg: { value: { id: 'o1', name: 'Acme', timezone: 'Asia/Taipei', checkin: { transfer_enabled: true, location_tracking_enabled: true } } },
  isAdmin: { value: true },
  isAuthenticated: { value: true },
  ensureLoaded: vi.fn(async () => {}),
  refresh: vi.fn(async () => {}),
}
mockNuxtImport('useAuth', () => () => fakeAuth)
mockNuxtImport('useCheckin', () => () => ({
  listUserEvents: vi.fn(async () => []),
}))

const listMock = vi.fn()
mockNuxtImport('useLocationPings', () => () => ({
  list: listMock,
}))

describe('Trajectory page', () => {
  it('shows empty state and no map when API returns zero pings', async () => {
    listMock.mockResolvedValueOnce([])
    const wrapper = await mountSuspended(TrajectoryPage, {
      route: { params: { appUserId: 'u1' }, query: { date: '2026-05-05' } },
    })
    await new Promise(resolve => setTimeout(resolve, 0))
    await wrapper.vm.$nextTick()

    expect(wrapper.text()).toContain('該日無軌跡資料')
    expect(wrapper.find('[data-testid="trajectory-empty"]').exists()).toBe(true)
    expect(wrapper.find('[data-testid="trajectory-map"]').exists()).toBe(false)
  })

  it('mounts map container when pings are present', async () => {
    listMock.mockResolvedValueOnce([
      {
        id: 'p1',
        app_user_id: 'u1',
        lat: 25.04,
        lng: 121.55,
        occurred_at_client: '2026-05-05T10:00:00+08:00',
        occurred_at_server: '2026-05-05T02:00:00Z',
      },
    ])
    const wrapper = await mountSuspended(TrajectoryPage, {
      route: { params: { appUserId: 'u1' }, query: { date: '2026-05-05' } },
    })
    await new Promise(resolve => setTimeout(resolve, 50))
    await wrapper.vm.$nextTick()

    expect(wrapper.find('[data-testid="trajectory-empty"]').exists()).toBe(false)
    expect(wrapper.find('[data-testid="trajectory-map"]').exists()).toBe(true)
  })
})
