import { beforeEach, describe, expect, it, vi } from 'vitest'
import { mockNuxtImport, mountSuspended } from '@nuxt/test-utils/runtime'

import TrajectoryPage from '~/pages/checkin/[appUserId]/trajectory.vue'

// Mock Leaflet — we don't run a real map in tests; we only assert the
// component-level state (empty / has-data flag → DOM container presence) plus
// the geometry the page hands to Leaflet. This keeps the test framework
// agnostic of happy-dom's DOM coverage gaps (createPane /
// getBoundingClientRect quirks).
// Spies are typed on their first parameter so `mock.calls[i][0]` carries the
// geometry type instead of `never` under `nuxt typecheck`.
const leafletSpies = vi.hoisted(() => ({
  polyline: vi.fn((_latlngs: [number, number][], _opts?: unknown) => ({ addTo: vi.fn() })),
  circleMarker: vi.fn((_latlng: [number, number], _opts?: unknown) => ({
    bindPopup: vi.fn(() => ({ addTo: vi.fn() })),
  })),
}))

// NOTE: the page does `leaflet = await import('leaflet')` and then reads
// `leaflet.map` / `leaflet.polyline` off the module namespace directly, so the
// members must exist at the TOP level of the mock, not only under `default`.
// A default-only mock leaves `L.map` undefined, `renderMap()` throws inside an
// async watcher, and the throw is swallowed — the map silently never draws
// while DOM-presence assertions still pass.
const leafletModule = {
  map: vi.fn(() => ({
    remove: vi.fn(),
    eachLayer: vi.fn(),
    removeLayer: vi.fn(),
    fitBounds: vi.fn(),
  })),
  tileLayer: vi.fn(() => ({ addTo: vi.fn() })),
  polyline: leafletSpies.polyline,
  circleMarker: leafletSpies.circleMarker,
  latLngBounds: vi.fn(),
  TileLayer: class {},
}

vi.mock('leaflet', () => ({ ...leafletModule, default: leafletModule }))
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

const listUserEventsMock = vi.fn()
mockNuxtImport('useCheckin', () => () => ({
  listUserEvents: listUserEventsMock,
}))

const listMock = vi.fn()
mockNuxtImport('useLocationPings', () => () => ({
  list: listMock,
}))

function ping(id: string, iso: string, lat: number, lng: number) {
  return {
    id,
    app_user_id: 'u1',
    lat,
    lng,
    occurred_at_client: iso,
    occurred_at_server: iso,
  }
}

function event(
  id: string,
  eventType: string,
  iso: string,
  lat: number,
  lng: number,
  source = 'app',
) {
  return {
    id,
    app_user_id: 'u1',
    event_type: eventType,
    occurred_at_client: iso,
    occurred_at_server: iso,
    source,
    initiated_by_kind: source === 'admin_force' ? 'dashboard_user' : 'app_user',
    initiated_by_id: 'x1',
    location: { coordinates: { lat, lng } },
    has_skew_warning: false,
  }
}

/**
 * Resolve after a tick so the fetch settles AFTER the component mounts.
 *
 * This matters: the page renders its map from a `watch(hasData)`, which only
 * fires on a *change*. A mock that resolves synchronously during Suspense
 * leaves `hasData` already `true` at mount, the watcher never fires, and no
 * map is ever drawn — an artifact of instant mocks, not of the page. Real
 * network latency always flips the flag post-mount.
 */
function defer<T>(value: T): Promise<T> {
  return new Promise(resolve => setTimeout(() => resolve(value), 10))
}

function resolvePings(pings: ReturnType<typeof ping>[]) {
  listMock.mockImplementation(() => defer(pings))
}

function resolveEvents(events: ReturnType<typeof event>[]) {
  listUserEventsMock.mockImplementation(() => defer(events))
}

async function mountPage() {
  const wrapper = await mountSuspended(TrajectoryPage, {
    route: { params: { appUserId: 'u1' }, query: { date: '2026-05-05' } },
  })
  await new Promise(resolve => setTimeout(resolve, 80))
  await wrapper.vm.$nextTick()
  return wrapper
}

/** The [lat,lng] pairs the page passed to L.polyline, in call order. */
function drawnSegments(): [number, number][][] {
  return leafletSpies.polyline.mock.calls.map(c => c[0])
}

/** The [lat,lng] pairs the page passed to L.circleMarker, in call order. */
function drawnMarkers(): [number, number][] {
  return leafletSpies.circleMarker.mock.calls.map(c => c[0])
}

describe('Trajectory page', () => {
  beforeEach(() => {
    leafletSpies.polyline.mockClear()
    leafletSpies.circleMarker.mockClear()
    leafletModule.map.mockClear()
    listMock.mockReset()
    listUserEventsMock.mockReset()
    resolvePings([])
    resolveEvents([])
  })

  it('shows empty state and no map when there are zero merged points', async () => {
    const wrapper = await mountPage()

    expect(wrapper.text()).toContain('該日無軌跡資料')
    expect(wrapper.find('[data-testid="trajectory-empty"]').exists()).toBe(true)
    expect(wrapper.find('[data-testid="trajectory-map"]').exists()).toBe(false)
    expect(leafletModule.map).not.toHaveBeenCalled()
  })

  it('mounts map container when pings are present', async () => {
    resolvePings([ping('p1', '2026-05-05T10:00:00+08:00', 25.04, 121.55)])
    const wrapper = await mountPage()

    expect(wrapper.find('[data-testid="trajectory-empty"]').exists()).toBe(false)
    expect(wrapper.find('[data-testid="trajectory-map"]').exists()).toBe(true)
  })

  it('fetches events range-scoped to the resolved day, not as an unranged page', async () => {
    await mountPage()

    expect(listUserEventsMock).toHaveBeenCalledWith('u1', {
      from: '2026-05-05T00:00:00+08:00',
      to: '2026-05-06T00:00:00+08:00',
      limit: 200,
    })
  })

  it('draws the line from the clock-in coordinate to the clock-out coordinate', async () => {
    resolvePings([
      ping('p1', '2026-05-05T09:00:00+08:00', 25.10, 121.10),
      ping('p2', '2026-05-05T12:00:00+08:00', 25.20, 121.20),
    ])
    resolveEvents([
      event('e_out', 'clock_out', '2026-05-05T17:00:00+08:00', 25.90, 121.90),
      event('e_in', 'clock_in', '2026-05-05T08:00:00+08:00', 25.00, 121.00),
    ])
    await mountPage()

    const segments = drawnSegments()
    // 4 merged points → 3 segments.
    expect(segments).toHaveLength(3)
    expect(segments[0][0]).toEqual([25.00, 121.00])
    expect(segments[segments.length - 1][1]).toEqual([25.90, 121.90])
  })

  it('draws a line for two events with zero pings', async () => {
    resolveEvents([
      event('e_in', 'clock_in', '2026-05-05T08:00:00+08:00', 25.00, 121.00),
      event('e_out', 'clock_out', '2026-05-05T17:00:00+08:00', 25.50, 121.50),
    ])
    const wrapper = await mountPage()

    expect(wrapper.find('[data-testid="trajectory-map"]').exists()).toBe(true)
    const segments = drawnSegments()
    expect(segments).toHaveLength(1)
    expect(segments[0]).toEqual([[25.00, 121.00], [25.50, 121.50]])
  })

  it('renders the map but no line for a single merged point', async () => {
    resolveEvents([event('e_in', 'clock_in', '2026-05-05T08:00:00+08:00', 25.00, 121.00)])
    const wrapper = await mountPage()

    expect(wrapper.find('[data-testid="trajectory-map"]').exists()).toBe(true)
    expect(wrapper.find('[data-testid="trajectory-empty"]').exists()).toBe(false)
    expect(drawnSegments()).toHaveLength(0)
    expect(drawnMarkers()).toEqual([[25.00, 121.00]])
  })

  it('excludes an admin_force clock_out from the line but still marks it', async () => {
    resolvePings([ping('p1', '2026-05-05T09:00:00+08:00', 25.10, 121.10)])
    resolveEvents([
      event('e_in', 'clock_in', '2026-05-05T08:00:00+08:00', 25.00, 121.00),
      // Forced checkout at 23:00 carrying a location copied from an earlier
      // event — a vertex here would draw a fabricated leg back to it.
      event('e_forced', 'clock_out', '2026-05-05T23:00:00+08:00', 24.00, 120.00, 'admin_force'),
    ])
    await mountPage()

    const segments = drawnSegments()
    expect(segments).toHaveLength(1)
    expect(segments[0]).toEqual([[25.00, 121.00], [25.10, 121.10]])
    expect(segments.flat()).not.toContainEqual([24.00, 120.00])

    // The marker is still drawn where the admin closed the shift.
    expect(drawnMarkers()).toContainEqual([24.00, 120.00])
  })

  it('includes legacy_backfill events in the line', async () => {
    resolveEvents([
      event('l1', 'clock_in', '2026-05-05T08:00:00+08:00', 25.00, 121.00, 'legacy_backfill'),
      event('l2', 'clock_out', '2026-05-05T17:00:00+08:00', 25.50, 121.50, 'legacy_backfill'),
    ])
    await mountPage()

    expect(drawnSegments()).toHaveLength(1)
  })
})
