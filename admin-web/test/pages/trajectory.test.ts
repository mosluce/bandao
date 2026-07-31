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
  // Typed on the container parameter so `map.mock.calls[i][0]` carries the
  // element instead of `never` — the date-switch tests assert on it.
  map: vi.fn((_container?: unknown) => ({
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
  await settle(wrapper)
  return wrapper
}

async function settle(wrapper: Awaited<ReturnType<typeof mountSuspended>>) {
  await new Promise(resolve => setTimeout(resolve, 80))
  await wrapper.vm.$nextTick()
}

/**
 * Drive the date picker the way an admin does, then let the refetch settle.
 *
 * The blank-map bug lived exactly here: the page kept rendering off a
 * `watch(hasData)` that only fires on a *change*, so a day-with-data →
 * day-with-data switch left the new container empty until a page reload.
 */
async function selectDate(
  wrapper: Awaited<ReturnType<typeof mountSuspended>>,
  date: string,
) {
  await wrapper.find('input[name="date-picker"]').setValue(date)
  await settle(wrapper)
}

/** The [lat,lng] pairs the page passed to L.polyline, in call order. */
function drawnSegments(): [number, number][][] {
  return leafletSpies.polyline.mock.calls.map(c => c[0])
}

/** The [lat,lng] pairs the page passed to L.circleMarker, in call order. */
function drawnMarkers(): [number, number][] {
  return leafletSpies.circleMarker.mock.calls.map(c => c[0])
}

/**
 * The heart of the blank-map regression: the Leaflet instance must be bound to
 * the container element that is *currently* in the document.
 *
 * Drawn-geometry assertions alone cannot catch the bug — the old code happily
 * called `L.polyline` with the new day's coordinates, it just fed them to a map
 * whose container Vue had already unmounted. Only comparing the element the map
 * was built against with the live one distinguishes the two.
 */
function expectMapBoundToLiveContainer(
  wrapper: Awaited<ReturnType<typeof mountSuspended>>,
) {
  const liveEl = wrapper.find('[data-testid="trajectory-map"]').element
  const builtAgainst = leafletModule.map.mock.calls.at(-1)?.[0]
  expect(builtAgainst).toBe(liveEl)
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
    // The container is permanently mounted now — unmounting it is what left
    // Leaflet bound to a detached node. "No map" means Leaflet was never
    // instantiated, not that the element is absent.
    expect(wrapper.find('[data-testid="trajectory-map"]').exists()).toBe(true)
    expect(leafletModule.map).not.toHaveBeenCalled()
    expect(wrapper.find('[data-testid="trajectory-legend"]').exists()).toBe(false)
  })

  it('instantiates Leaflet when pings are present', async () => {
    resolvePings([ping('p1', '2026-05-05T10:00:00+08:00', 25.04, 121.55)])
    const wrapper = await mountPage()

    expect(wrapper.find('[data-testid="trajectory-empty"]').exists()).toBe(false)
    expect(leafletModule.map).toHaveBeenCalledTimes(1)
    expect(wrapper.find('[data-testid="trajectory-legend"]').exists()).toBe(true)
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
    await mountPage()

    expect(leafletModule.map).toHaveBeenCalledTimes(1)
    const segments = drawnSegments()
    expect(segments).toHaveLength(1)
    expect(segments[0]).toEqual([[25.00, 121.00], [25.50, 121.50]])
  })

  it('renders the map but no line for a single merged point', async () => {
    resolveEvents([event('e_in', 'clock_in', '2026-05-05T08:00:00+08:00', 25.00, 121.00)])
    const wrapper = await mountPage()

    expect(leafletModule.map).toHaveBeenCalledTimes(1)
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

  describe('changing the date', () => {
    function dayA() {
      return [
        event('a_in', 'clock_in', '2026-05-05T08:00:00+08:00', 25.00, 121.00),
        event('a_out', 'clock_out', '2026-05-05T17:00:00+08:00', 25.50, 121.50),
      ]
    }

    function dayB() {
      return [
        event('b_in', 'clock_in', '2026-05-06T08:00:00+08:00', 24.00, 120.00),
        event('b_out', 'clock_out', '2026-05-06T17:00:00+08:00', 24.50, 120.50),
      ]
    }

    it('redraws when both the old and the new date have data', async () => {
      resolveEvents(dayA())
      const wrapper = await mountPage()
      expect(drawnSegments()).toHaveLength(1)

      leafletSpies.polyline.mockClear()
      leafletSpies.circleMarker.mockClear()
      resolveEvents(dayB())
      await selectDate(wrapper, '2026-05-06')

      expect(drawnSegments()).toEqual([[[24.00, 120.00], [24.50, 120.50]]])
      expect(drawnMarkers()).toEqual([[24.00, 120.00], [24.50, 120.50]])
      // The retained instance is reused, not rebuilt against a new container.
      expect(leafletModule.map).toHaveBeenCalledTimes(1)
      expectMapBoundToLiveContainer(wrapper)
    })

    it('recovers after stepping onto a date with zero merged points', async () => {
      resolveEvents(dayA())
      const wrapper = await mountPage()

      resolveEvents([])
      await selectDate(wrapper, '2026-05-06')
      expect(wrapper.find('[data-testid="trajectory-empty"]').exists()).toBe(true)
      expect(wrapper.find('[data-testid="trajectory-legend"]').exists()).toBe(false)

      leafletSpies.polyline.mockClear()
      resolveEvents(dayB())
      await selectDate(wrapper, '2026-05-07')

      expect(wrapper.find('[data-testid="trajectory-empty"]').exists()).toBe(false)
      expect(drawnSegments()).toEqual([[[24.00, 120.00], [24.50, 120.50]]])
      expect(leafletModule.map).toHaveBeenCalledTimes(1)
      expectMapBoundToLiveContainer(wrapper)
    })

    it('renders the next date after a failed fetch', async () => {
      listMock.mockImplementation(() => Promise.reject(new Error('網路錯誤')))
      const wrapper = await mountPage()
      expect(wrapper.text()).toContain('網路錯誤')

      resolvePings([])
      resolveEvents(dayB())
      await selectDate(wrapper, '2026-05-06')

      expect(wrapper.text()).not.toContain('網路錯誤')
      expect(drawnSegments()).toEqual([[[24.00, 120.00], [24.50, 120.50]]])
    })
  })
})
