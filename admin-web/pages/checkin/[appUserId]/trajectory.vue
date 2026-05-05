<script setup lang="ts">
import type { CheckinEventDto, LocationPingDto } from '~/types/api'
import { ApiError } from '~/types/api'
import { dateToOrgRange } from '~/utils/orgTimeRange'

definePageMeta({ middleware: 'auth' })

const auth = useAuth()
const checkin = useCheckin()
const locationPings = useLocationPings()
const route = useRoute()
const router = useRouter()
const config = useRuntimeConfig()

const appUserId = computed(() => String(route.params.appUserId))
const orgTz = computed(() => auth.currentOrg.value?.timezone || 'Asia/Taipei')

function todayInOrgTz(tz: string): string {
  // YYYY-MM-DD in Org timezone, regardless of viewer's local tz.
  const fmt = new Intl.DateTimeFormat('en-CA', {
    timeZone: tz,
    year: 'numeric',
    month: '2-digit',
    day: '2-digit',
  })
  return fmt.format(new Date())
}

const dateInput = ref<string>(
  typeof route.query.date === 'string' && /^\d{4}-\d{2}-\d{2}$/.test(route.query.date)
    ? route.query.date
    : todayInOrgTz(orgTz.value),
)

const loading = ref(true)
const error = ref('')
const pings = ref<LocationPingDto[]>([])
const events = ref<CheckinEventDto[]>([])

const mapContainer = ref<HTMLElement | null>(null)
let mapInstance: any = null
let leaflet: any = null

const hasData = computed(() => pings.value.length > 0)

const exportModalOpen = ref(false)
const exportFrom = ref<string>(dateInput.value)
const exportTo = ref<string>(dateInput.value)
const exportError = ref('')

watch([dateInput, () => auth.currentOrg.value?.id], () => loadDay())

async function loadDay() {
  if (!auth.currentOrg.value) return
  loading.value = true
  error.value = ''
  try {
    const range = dateToOrgRange(dateInput.value, orgTz.value)
    const [pingsRes, eventsRes] = await Promise.all([
      locationPings.list({
        appUserId: appUserId.value,
        params: { from: range.from, to: range.to, limit: 1000 },
      }),
      // events list — server returns newest-first, single page covers a day.
      checkin.listUserEvents(appUserId.value, { limit: 100 }),
    ])
    // Sort pings ascending for the polyline.
    pings.value = [...pingsRes].sort((a, b) =>
      a.occurred_at_client.localeCompare(b.occurred_at_client),
    )
    // Filter events to the same day-range.
    events.value = eventsRes.filter((e) => {
      const t = e.occurred_at_client
      return t >= range.from && t < range.to
    })
  }
  catch (err) {
    if (err instanceof ApiError) {
      error.value = err.code === 'INVALID_RANGE'
        ? '日期範圍超過上限或格式錯誤'
        : err.message
    }
    else {
      error.value = err instanceof Error ? err.message : '載入失敗'
    }
    pings.value = []
    events.value = []
  }
  finally {
    loading.value = false
  }
}

async function ensureLeaflet() {
  if (leaflet) return leaflet
  // dynamic import — Leaflet pulls window/document refs at module top-level
  // in some bundles; lazy-load only when we actually have data to render.
  leaflet = await import('leaflet')
  // CSS import via dynamic import for SPA mode.
  await import('leaflet/dist/leaflet.css')
  return leaflet
}

watch(hasData, async (next) => {
  if (next) {
    await nextTick()
    await renderMap()
  }
  else {
    teardownMap()
  }
})

watch([pings, events], async () => {
  if (hasData.value && mapInstance) {
    redrawLayers()
  }
}, { deep: false })

async function renderMap() {
  if (!mapContainer.value) return
  const L = await ensureLeaflet()
  if (mapInstance) {
    mapInstance.remove()
    mapInstance = null
  }
  mapInstance = L.map(mapContainer.value)
  L.tileLayer('https://{s}.basemaps.cartocdn.com/light_all/{z}/{x}/{y}.png', {
    maxZoom: 19,
    attribution: '© OpenStreetMap contributors © CARTO',
  }).addTo(mapInstance)
  redrawLayers()
}

function redrawLayers() {
  if (!mapInstance || !leaflet) return
  const L = leaflet
  // Remove existing polyline / markers (cheap: clear all non-tile layers).
  mapInstance.eachLayer((layer: any) => {
    if (!(layer instanceof L.TileLayer)) {
      mapInstance.removeLayer(layer)
    }
  })

  const points: [number, number][] = pings.value.map(p => [p.lat, p.lng])
  if (points.length > 0) {
    L.polyline(points, { color: '#1f2937', weight: 3 }).addTo(mapInstance)
  }

  const eventColor: Record<string, string> = {
    clock_in: '#15803d',
    clock_out: '#475569',
    transfer_in: '#b45309',
    transfer_out: '#b45309',
  }
  const markerLatLngs: [number, number][] = []
  for (const e of events.value) {
    const lat = e.location.coordinates.lat
    const lng = e.location.coordinates.lng
    const color = eventColor[e.event_type] || '#475569'
    L.circleMarker([lat, lng], {
      radius: 7,
      color,
      fillColor: color,
      fillOpacity: 0.9,
      weight: 2,
    })
      .bindPopup(`${eventLabel(e.event_type)}<br>${e.occurred_at_client}`)
      .addTo(mapInstance)
    markerLatLngs.push([lat, lng])
  }

  const allLatLngs = points.concat(markerLatLngs)
  if (allLatLngs.length > 0) {
    const bounds = L.latLngBounds(allLatLngs)
    mapInstance.fitBounds(bounds, { padding: [20, 20] })
  }
}

function eventLabel(t: CheckinEventDto['event_type']): string {
  return t === 'clock_in' ? '上班' : t === 'clock_out' ? '下班' : t === 'transfer_in' ? '轉入' : '轉出'
}

function teardownMap() {
  if (mapInstance) {
    mapInstance.remove()
    mapInstance = null
  }
}

onBeforeUnmount(teardownMap)

watch(dateInput, (v) => {
  // Keep URL ?date=… in sync.
  router.replace({ query: { ...route.query, date: v } })
})

function focusDatePicker() {
  // Anchor for the empty state's "換日期" affordance.
  const el = document.querySelector<HTMLInputElement>('input[type="date"][name="date-picker"]')
  el?.focus()
  el?.showPicker?.()
}

function openExport() {
  exportFrom.value = dateInput.value
  exportTo.value = dateInput.value
  exportError.value = ''
  exportModalOpen.value = true
}

function closeExport() {
  exportModalOpen.value = false
}

function validateExportRange(): string {
  if (!exportFrom.value || !exportTo.value) return '請選擇起訖日期'
  const fromMs = Date.parse(`${exportFrom.value}T00:00:00Z`)
  const toMs = Date.parse(`${exportTo.value}T00:00:00Z`)
  if (Number.isNaN(fromMs) || Number.isNaN(toMs)) return '日期格式錯誤'
  if (toMs < fromMs) return '結束日期不可早於起始日期'
  const spanDays = (toMs - fromMs) / 86_400_000
  if (spanDays > 90) return '時間區間最多 90 天'
  return ''
}

function confirmExport() {
  const err = validateExportRange()
  if (err) {
    exportError.value = err
    return
  }
  const range = dateToOrgRange(exportFrom.value, orgTz.value)
  // For `to` we want the *end* of the to-date — use start of next day.
  const toRange = dateToOrgRange(exportTo.value, orgTz.value)
  const url = new URL(`${config.public.apiBaseUrl}/checkin/users/${appUserId.value}/locations/export`)
  url.searchParams.set('from', range.from)
  url.searchParams.set('to', toRange.to)

  const anchor = document.createElement('a')
  anchor.href = url.toString()
  anchor.target = '_blank'
  anchor.rel = 'noopener'
  document.body.appendChild(anchor)
  anchor.click()
  document.body.removeChild(anchor)

  closeExport()
}

if (auth.isAdmin.value && auth.currentOrg.value) {
  loadDay()
}
</script>

<template>
  <main class="min-h-screen px-4 py-10">
    <div class="max-w-5xl mx-auto space-y-4">
      <header class="flex items-center justify-between gap-3">
        <div class="min-w-0">
          <NuxtLink
            :to="`/checkin/${appUserId}`"
            class="text-xs text-slate-500 hover:text-slate-900"
          >
            ← 返回事件歷史
          </NuxtLink>
          <h1 class="mt-1 text-2xl font-semibold text-slate-900">
            軌跡
          </h1>
          <p class="text-sm text-slate-500 truncate">
            {{ auth.currentOrg.value?.name }} · App 使用者 <code class="font-mono">{{ appUserId }}</code>
          </p>
        </div>
        <div class="flex shrink-0 items-center gap-2">
          <input
            v-model="dateInput"
            type="date"
            name="date-picker"
            class="rounded-md border border-slate-300 px-3 py-2 text-sm"
          >
          <button
            type="button"
            class="rounded-md border border-slate-300 bg-white px-3 py-2 text-sm font-medium text-slate-700 hover:bg-slate-50"
            @click="openExport"
          >
            匯出 xlsx
          </button>
        </div>
      </header>

      <div
        v-if="loading"
        class="rounded-xl border border-slate-200 bg-white p-12 text-center text-sm text-slate-500"
      >
        載入軌跡中...
      </div>

      <div
        v-else-if="error"
        class="rounded-xl border border-red-200 bg-red-50 p-4 text-sm text-red-700"
      >
        {{ error }}
      </div>

      <div
        v-else-if="!hasData"
        class="rounded-xl border border-slate-200 bg-white p-12 text-center"
        data-testid="trajectory-empty"
      >
        <p class="text-sm text-slate-600">
          該日無軌跡資料
        </p>
        <button
          type="button"
          class="mt-3 text-sm text-slate-700 hover:text-slate-900 underline"
          @click="focusDatePicker"
        >
          換日期
        </button>
      </div>

      <div v-else class="rounded-xl border border-slate-200 overflow-hidden">
        <div
          ref="mapContainer"
          class="h-[600px] w-full"
          data-testid="trajectory-map"
        />
      </div>
    </div>

    <Teleport to="body">
      <div
        v-if="exportModalOpen"
        class="fixed inset-0 z-[1100] flex items-center justify-center bg-slate-900/40 px-4"
      >
        <div class="w-full max-w-md rounded-xl bg-white p-6 shadow-lg space-y-4">
        <h2 class="text-lg font-semibold text-slate-900">
          匯出軌跡 xlsx
        </h2>
        <p class="text-xs text-slate-500">
          時間區間最多 90 天。
        </p>
        <div class="space-y-2">
          <label class="block text-sm">
            <span class="text-slate-700">起始日期</span>
            <input
              v-model="exportFrom"
              type="date"
              class="mt-1 w-full rounded-md border border-slate-300 px-3 py-2"
            >
          </label>
          <label class="block text-sm">
            <span class="text-slate-700">結束日期</span>
            <input
              v-model="exportTo"
              type="date"
              class="mt-1 w-full rounded-md border border-slate-300 px-3 py-2"
            >
          </label>
        </div>
        <p
          v-if="exportError"
          class="text-xs text-red-600"
        >
          {{ exportError }}
        </p>
        <div class="flex justify-end gap-2">
          <button
            type="button"
            class="rounded-md border border-slate-300 bg-white px-3 py-2 text-sm font-medium text-slate-700 hover:bg-slate-50"
            @click="closeExport"
          >
            取消
          </button>
          <button
            type="button"
            class="rounded-md bg-slate-900 px-3 py-2 text-sm font-medium text-white hover:bg-slate-700"
            @click="confirmExport"
          >
            匯出
          </button>
        </div>
        </div>
      </div>
    </Teleport>
  </main>
</template>
