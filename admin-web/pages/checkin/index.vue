<script setup lang="ts">
import type { CheckinUserBoardRowDto } from '~/types/api'
import { ApiError } from '~/types/api'

definePageMeta({ middleware: 'auth' })

const auth = useAuth()
const checkin = useCheckin()

const rows = ref<CheckinUserBoardRowDto[]>([])
const loading = ref(true)
const loadError = ref('')
const lastRefreshed = ref<Date | null>(null)

const forceTargetId = ref<string | null>(null)
const forceReason = ref('')
const forcing = ref(false)
const forceError = ref('')

const orgTz = computed(() => auth.currentOrg.value?.timezone)

const onSite = computed(() => rows.value.filter(r => r.status === 'on_site'))
const inTransit = computed(() => rows.value.filter(r => r.status === 'in_transit'))
const offDuty = computed(() => rows.value.filter(r => r.status === 'off_duty'))

let pollHandle: ReturnType<typeof setInterval> | null = null

async function load() {
  loadError.value = ''
  try {
    rows.value = await checkin.listUsers()
    lastRefreshed.value = new Date()
  }
  catch (err) {
    loadError.value = err instanceof Error ? err.message : '載入失敗'
  }
  finally {
    loading.value = false
  }
}

watch(() => auth.currentOrg.value?.id, (newId, oldId) => {
  if (newId && newId !== oldId) {
    loading.value = true
    load()
  }
})

onMounted(() => {
  pollHandle = setInterval(() => { load() }, 30_000)
})
onBeforeUnmount(() => {
  if (pollHandle) clearInterval(pollHandle)
  pollHandle = null
})

function openForce(id: string) {
  forceTargetId.value = id
  forceReason.value = ''
  forceError.value = ''
}

function cancelForce() {
  forceTargetId.value = null
  forceReason.value = ''
  forceError.value = ''
}

async function confirmForce() {
  const id = forceTargetId.value
  if (!id) return
  forceError.value = ''
  forcing.value = true
  try {
    await checkin.forceCheckout(id, forceReason.value.trim() || undefined)
    await load()
    cancelForce()
  }
  catch (err) {
    forceError.value = friendlyForceError(err)
  }
  finally {
    forcing.value = false
  }
}

function friendlyForceError(err: unknown): string {
  if (!(err instanceof ApiError)) {
    return err instanceof Error ? err.message : '操作失敗'
  }
  switch (err.code) {
    case 'NOT_ON_DUTY':
      return '此 App 使用者目前已下班'
    case 'NOT_FOUND':
      return '找不到此 App 使用者'
    case 'FORBIDDEN':
      return '只有管理員可以強制收班'
    default:
      return err.message
  }
}

function statusBadgeClass(s: CheckinUserBoardRowDto['status']): string {
  if (s === 'on_site') return 'bg-green-100 text-green-800'
  if (s === 'in_transit') return 'bg-amber-100 text-amber-800'
  return 'bg-slate-200 text-slate-700'
}

function lastEventSummary(row: CheckinUserBoardRowDto): string {
  const e = row.last_event
  if (!e) return '—'
  const place = e.location.manual_label
    || e.location.region_name
    || `${e.location.coordinates.lat.toFixed(4)}, ${e.location.coordinates.lng.toFixed(4)}`
  const type = e.event_type === 'clock_in'
    ? '上班'
    : e.event_type === 'clock_out'
      ? '下班'
      : e.event_type === 'transfer_out' ? '轉出' : '轉入'
  return `${type} @ ${place}`
}

await load()
</script>

<template>
  <main class="min-h-screen px-4 py-10">
    <div class="max-w-5xl mx-auto space-y-6">
      <header>
        <h1 class="text-2xl font-semibold text-slate-900">
          打卡看板
        </h1>
        <p class="text-sm text-slate-500 truncate">
          {{ auth.currentOrg.value?.name }} · 在班 / 移動中 / 下班 即時看板，每 30 秒自動刷新
          <span
            v-if="lastRefreshed"
            class="ml-2 text-slate-400"
          >
            （{{ formatInOrgTz(lastRefreshed.toISOString(), orgTz) }} 更新）
          </span>
        </p>
      </header>

      <p
        v-if="loadError"
        class="text-sm text-red-600"
      >
        {{ loadError }}
      </p>

      <p
        v-if="loading"
        class="text-sm text-slate-500"
      >
        載入中…
      </p>

      <template v-else>
        <section
          v-for="(group, idx) in [
            { title: '在班', rows: onSite, force: true, dot: 'bg-green-500' },
            { title: '移動中', rows: inTransit, force: true, dot: 'bg-amber-500' },
            { title: '下班', rows: offDuty, force: false, dot: 'bg-slate-300' },
          ]"
          :key="idx"
          class="rounded-xl border border-slate-200 bg-white shadow-sm"
        >
          <header class="flex items-center gap-2 px-6 py-3 border-b border-slate-100">
            <span
              class="inline-block h-2 w-2 rounded-full"
              :class="group.dot"
            />
            <h2 class="text-sm font-semibold text-slate-900">
              {{ group.title }}
              <span class="ml-1 text-slate-400">{{ group.rows.length }}</span>
            </h2>
          </header>

          <p
            v-if="group.rows.length === 0"
            class="px-6 py-4 text-sm text-slate-400 italic"
          >
            無
          </p>

          <ul
            v-else
            class="divide-y divide-slate-100"
          >
            <li
              v-for="r in group.rows"
              :key="r.user.id"
              class="flex items-center justify-between gap-4 px-6 py-3"
            >
              <div class="min-w-0">
                <NuxtLink
                  :to="`/checkin/${r.user.id}`"
                  class="font-medium text-slate-900 hover:underline"
                >
                  {{ r.user.display_name }}
                </NuxtLink>
                <span class="ml-1 font-mono text-xs text-slate-500">{{ r.user.username }}</span>
                <span
                  v-if="r.has_skew_warning"
                  class="ml-2 inline-block rounded bg-amber-100 px-1.5 py-0.5 text-xs text-amber-800"
                  title="最新事件的 client / server 時間差異 > 1 小時"
                >⚠ skew</span>
                <p class="mt-0.5 text-xs text-slate-500">
                  <template v-if="r.status !== 'off_duty'">
                    上班 {{ shiftDuration(r.current_shift_started_at) }} ·
                  </template>
                  {{ lastEventSummary(r) }}
                </p>
              </div>
              <div class="flex shrink-0 items-center gap-2">
                <span
                  class="rounded px-1.5 py-0.5 text-xs"
                  :class="statusBadgeClass(r.status)"
                >{{ group.title }}</span>
                <button
                  v-if="group.force && auth.isAdmin.value"
                  type="button"
                  class="rounded-md border border-red-300 bg-white px-3 py-1.5 text-xs font-medium text-red-700 hover:bg-red-50"
                  @click="openForce(r.user.id)"
                >
                  強制收班
                </button>
              </div>
            </li>
          </ul>
        </section>
      </template>

      <div
        v-if="forceTargetId"
        class="fixed inset-0 z-50 flex items-center justify-center bg-slate-900/40 px-4"
      >
        <div class="w-full max-w-md rounded-xl bg-white p-6 shadow-xl space-y-4">
          <div>
            <h3 class="text-lg font-semibold text-slate-900">
              強制收班
            </h3>
            <p class="text-sm text-slate-500 mt-1">
              將寫入一筆 <code class="rounded bg-slate-100 px-1">clock_out</code> 事件，標註為「管理員強制收班」。可選填原因供後續查詢。
            </p>
          </div>

          <div>
            <label
              for="forceReason"
              class="block text-sm font-medium text-slate-700 mb-1"
            >原因（選填）</label>
            <input
              id="forceReason"
              v-model="forceReason"
              type="text"
              maxlength="240"
              :disabled="forcing"
              placeholder="例：員工忘記下班"
              class="w-full rounded-md border border-slate-300 px-3 py-2 text-sm focus:border-slate-900 focus:outline-none focus:ring-1 focus:ring-slate-900"
            >
          </div>

          <p
            v-if="forceError"
            class="text-sm text-red-600"
          >
            {{ forceError }}
          </p>

          <div class="flex justify-end gap-2">
            <button
              type="button"
              :disabled="forcing"
              class="rounded-md border border-slate-300 bg-white px-3 py-2 text-sm font-medium text-slate-700 hover:bg-slate-50"
              @click="cancelForce"
            >
              取消
            </button>
            <button
              type="button"
              :disabled="forcing"
              class="rounded-md bg-red-600 px-3 py-2 text-sm font-medium text-white hover:bg-red-700 disabled:opacity-60"
              @click="confirmForce"
            >
              {{ forcing ? '處理中…' : '確認強制收班' }}
            </button>
          </div>
        </div>
      </div>
    </div>
  </main>
</template>
