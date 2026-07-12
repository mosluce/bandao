<script setup lang="ts">
import type { CheckinEventDto } from '~/types/api'

definePageMeta({ middleware: 'auth' })

const auth = useAuth()
const checkin = useCheckin()
const route = useRoute()

const appUserId = computed(() => String(route.params.appUserId))
const events = ref<CheckinEventDto[]>([])
const loading = ref(true)
const loadError = ref('')
const reachedEnd = ref(false)
const loadingMore = ref(false)

const orgTz = computed(() => auth.currentOrg.value?.timezone)

const PAGE_SIZE = 50

async function loadFirstPage() {
  loadError.value = ''
  loading.value = true
  reachedEnd.value = false
  try {
    events.value = await checkin.listUserEvents(appUserId.value, { limit: PAGE_SIZE })
    if (events.value.length < PAGE_SIZE) reachedEnd.value = true
  }
  catch (err) {
    loadError.value = err instanceof Error ? err.message : '載入失敗'
  }
  finally {
    loading.value = false
  }
}

async function loadMore() {
  if (reachedEnd.value || loadingMore.value || events.value.length === 0) return
  loadingMore.value = true
  try {
    const before = events.value[events.value.length - 1].occurred_at_client
    const next = await checkin.listUserEvents(appUserId.value, { limit: PAGE_SIZE, before })
    if (next.length < PAGE_SIZE) reachedEnd.value = true
    events.value = events.value.concat(next)
  }
  catch (err) {
    loadError.value = err instanceof Error ? err.message : '載入失敗'
  }
  finally {
    loadingMore.value = false
  }
}

watch([() => auth.currentOrg.value?.id, appUserId], () => loadFirstPage())

function eventTypeLabel(t: CheckinEventDto['event_type']): string {
  return t === 'clock_in'
    ? '上班'
    : t === 'clock_out'
      ? '下班'
      : t === 'transfer_out' ? '轉出' : '轉入'
}

function eventBadgeClass(t: CheckinEventDto['event_type']): string {
  if (t === 'clock_in') return 'bg-green-100 text-green-800'
  if (t === 'clock_out') return 'bg-slate-200 text-slate-700'
  return 'bg-amber-100 text-amber-800'
}

function locationDisplay(e: CheckinEventDto): string {
  return e.location.manual_label
    || e.location.region_name
    || `${e.location.coordinates.lat.toFixed(5)}, ${e.location.coordinates.lng.toFixed(5)}`
}

await loadFirstPage()
</script>

<template>
  <main class="min-h-screen px-4 py-10">
    <div class="max-w-4xl mx-auto space-y-6">
      <header class="flex items-center justify-between gap-3">
        <div class="min-w-0">
          <NuxtLink
            to="/checkin"
            class="text-xs text-slate-500 hover:text-slate-900"
          >
            ← 返回打卡看板
          </NuxtLink>
          <h1 class="mt-1 text-2xl font-semibold text-slate-900">
            事件歷史
          </h1>
          <p class="text-sm text-slate-500 truncate">
            {{ auth.currentOrg.value?.name }} · App 使用者 ID <code class="font-mono">{{ appUserId }}</code>
          </p>
        </div>
        <div
          v-if="auth.currentOrg.value?.checkin.location_tracking_enabled"
          class="flex shrink-0 items-center gap-2"
        >
          <NuxtLink
            :to="`/checkin/${appUserId}/trajectory`"
            class="rounded-md border border-slate-300 bg-white px-3 py-2 text-sm font-medium text-slate-700 hover:bg-slate-50"
          >
            查看軌跡
          </NuxtLink>
        </div>
      </header>

      <p
        v-if="loadError"
        class="text-sm text-red-600"
      >
        {{ loadError }}
      </p>

      <p
        v-else-if="loading"
        class="text-sm text-slate-500"
      >
        載入中…
      </p>

      <p
        v-else-if="events.length === 0"
        class="rounded-xl border border-slate-200 bg-white px-6 py-8 text-center text-sm text-slate-500"
      >
        尚無事件。
      </p>

      <ul
        v-else
        class="divide-y divide-slate-200 rounded-xl border border-slate-200 bg-white shadow-sm"
      >
        <li
          v-for="e in events"
          :key="e.id"
          class="px-6 py-3"
        >
          <div class="flex items-center gap-2">
            <span
              class="rounded px-1.5 py-0.5 text-xs font-medium"
              :class="eventBadgeClass(e.event_type)"
            >{{ eventTypeLabel(e.event_type) }}</span>
            <span
              v-if="e.source === 'admin_force'"
              class="rounded bg-red-100 px-1.5 py-0.5 text-xs text-red-700"
              title="管理員強制收班"
            >強制</span>
            <span
              v-if="e.has_skew_warning"
              class="rounded bg-amber-100 px-1.5 py-0.5 text-xs text-amber-800"
              :title="`client / server 時差 > 1 小時 (server: ${formatInOrgTz(e.occurred_at_server, orgTz)})`"
            >⚠ skew</span>
            <span class="ml-auto text-xs text-slate-500">
              {{ formatInOrgTz(e.occurred_at_client, orgTz) }}
            </span>
          </div>
          <p class="mt-1 text-sm text-slate-700">
            {{ locationDisplay(e) }}
          </p>
          <p
            v-if="e.location.manual_label && e.location.region_name && e.location.manual_label !== e.location.region_name"
            class="text-xs text-slate-500"
          >
            （reverse geocoded：{{ e.location.region_name }}）
          </p>
          <p
            v-if="e.reason"
            class="mt-1 text-xs text-slate-600"
          >
            備註：{{ e.reason }}
          </p>
          <p class="mt-1 font-mono text-xs text-slate-400">
            {{ e.location.coordinates.lat.toFixed(5) }}, {{ e.location.coordinates.lng.toFixed(5) }}
            <template v-if="e.location.accuracy_meters !== undefined">
              · ±{{ Math.round(e.location.accuracy_meters) }}m
            </template>
          </p>
        </li>
      </ul>

      <div
        v-if="!loading && !reachedEnd && events.length > 0"
        class="text-center"
      >
        <button
          type="button"
          :disabled="loadingMore"
          class="rounded-md border border-slate-300 bg-white px-4 py-2 text-sm font-medium text-slate-700 hover:bg-slate-50 disabled:opacity-60"
          @click="loadMore"
        >
          {{ loadingMore ? '載入中…' : '載入更多' }}
        </button>
      </div>

      <p
        v-if="reachedEnd && events.length > 0"
        class="text-center text-xs text-slate-400"
      >
        — 沒有更多事件了 —
      </p>
    </div>
  </main>
</template>
