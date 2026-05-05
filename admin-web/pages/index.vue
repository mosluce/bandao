<script setup lang="ts">
import type { RotateCodeResponse } from '~/types/api'
import { ApiError } from '~/types/api'

definePageMeta({ middleware: 'auth' })

const auth = useAuth()
const api = useApi()
const orgSlug = useOrgSlug()

const copied = ref(false)
const rotating = ref(false)
const rotateError = ref('')
const showRotateConfirm = ref(false)

const slugInput = ref('')
const slugEditing = ref(false)
const slugSaving = ref(false)
const slugError = ref('')
const slugRetryAfter = ref<string | null>(null)
const showClearConfirm = ref(false)

const currentSlug = computed(() => auth.currentOrg.value?.slug ?? null)

const inviteIdentifier = computed(() => {
  const o = auth.currentOrg.value
  if (!o) return ''
  return o.slug || o.code
})

const inviteUrl = computed(() => {
  if (!inviteIdentifier.value) return ''
  if (typeof window === 'undefined') return ''
  return `${window.location.origin}/register?code=${inviteIdentifier.value}`
})

const slugFormatHint = '小寫英數字 2~24 個字元'

function describeSlugError(err: unknown): { message: string, retryAfter: string | null } {
  if (!(err instanceof ApiError)) {
    return { message: err instanceof Error ? err.message : '操作失敗', retryAfter: null }
  }
  switch (err.code) {
    case 'INVALID_SLUG_FORMAT':
      return { message: `格式不符（${slugFormatHint}）`, retryAfter: null }
    case 'SLUG_RESERVED':
      return { message: '此名稱為保留字，請改用其他名稱', retryAfter: null }
    case 'SLUG_TAKEN':
      return { message: '此名稱已被其他組織使用（或仍在 30 天 grace 期間）', retryAfter: null }
    case 'SLUG_CHANGE_TOO_SOON':
      return { message: '距離上次變更未滿 30 天', retryAfter: err.retryAfter }
    case 'FORBIDDEN':
      return { message: '只有管理員可以變更', retryAfter: null }
    default:
      return { message: err.message, retryAfter: null }
  }
}

function formatRetryAfter(iso: string): string {
  try {
    return new Date(iso).toLocaleString()
  }
  catch {
    return iso
  }
}

function startEditSlug() {
  slugInput.value = currentSlug.value ?? ''
  slugEditing.value = true
  slugError.value = ''
  slugRetryAfter.value = null
}

function cancelEditSlug() {
  slugEditing.value = false
  slugError.value = ''
  slugRetryAfter.value = null
}

async function saveSlug() {
  slugError.value = ''
  slugRetryAfter.value = null
  slugSaving.value = true
  try {
    await orgSlug.setOrgSlug(slugInput.value.trim().toLowerCase())
    await auth.refresh()
    slugEditing.value = false
  }
  catch (err) {
    const desc = describeSlugError(err)
    slugError.value = desc.message
    slugRetryAfter.value = desc.retryAfter
  }
  finally {
    slugSaving.value = false
  }
}

async function clearSlug() {
  slugError.value = ''
  slugRetryAfter.value = null
  slugSaving.value = true
  try {
    await orgSlug.clearOrgSlug()
    await auth.refresh()
    showClearConfirm.value = false
  }
  catch (err) {
    const desc = describeSlugError(err)
    slugError.value = desc.message
    slugRetryAfter.value = desc.retryAfter
  }
  finally {
    slugSaving.value = false
  }
}

async function copyCode() {
  const o = auth.currentOrg.value
  if (!o) return
  try {
    await navigator.clipboard.writeText(o.code)
    copied.value = true
    setTimeout(() => { copied.value = false }, 1500)
  }
  catch {
    // Clipboard may be unavailable in insecure contexts; ignore silently.
  }
}

async function copyInvite() {
  if (!inviteUrl.value) return
  try {
    await navigator.clipboard.writeText(inviteUrl.value)
    copied.value = true
    setTimeout(() => { copied.value = false }, 1500)
  }
  catch {
    // ignore
  }
}

async function rotateCode() {
  rotateError.value = ''
  rotating.value = true
  try {
    await api<RotateCodeResponse>('/orgs/me/code/rotate', { method: 'POST' })
    await auth.refresh()
    showRotateConfirm.value = false
  }
  catch (err) {
    rotateError.value = err instanceof Error ? err.message : '輪替失敗'
  }
  finally {
    rotating.value = false
  }
}

async function onLogout() {
  await auth.logout()
  await navigateTo('/login')
}

const orgSettings = useOrgSettings()
const joinRequests = useJoinRequests()
const transferToggleSaving = ref(false)
const transferToggleError = ref('')
const stateLockedCount = ref<number | null>(null)

const pendingJoinCount = ref(0)
let pendingJoinTimer: ReturnType<typeof setInterval> | null = null

async function refreshPendingJoinCount() {
  if (!auth.isAdmin.value || !auth.currentOrg.value) {
    pendingJoinCount.value = 0
    return
  }
  try {
    pendingJoinCount.value = await joinRequests.countOrgPending()
  }
  catch {
    // best-effort badge — don't surface errors here
  }
}

watch(
  [() => auth.currentOrg.value?.id, () => auth.isAdmin.value],
  () => refreshPendingJoinCount(),
  { immediate: true },
)

onMounted(() => {
  pendingJoinTimer = setInterval(refreshPendingJoinCount, 30_000)
})
onBeforeUnmount(() => {
  if (pendingJoinTimer) clearInterval(pendingJoinTimer)
})

const locationTrackingToggleSaving = ref(false)
const locationTrackingToggleError = ref('')

async function toggleTransfer() {
  if (!auth.currentOrg.value) return
  transferToggleError.value = ''
  stateLockedCount.value = null
  transferToggleSaving.value = true
  const target = !auth.currentOrg.value.checkin.transfer_enabled
  try {
    await orgSettings.update({ transfer_enabled: target })
    await auth.refresh()
  }
  catch (err) {
    if (err instanceof ApiError && err.code === 'STATE_LOCKED') {
      // ApiError doesn't carry structured body; pull from message if surfaced.
      transferToggleError.value = '目前有 App 使用者在班，需先全部下班才能調整此設定'
    }
    else if (err instanceof ApiError) {
      transferToggleError.value = err.code === 'FORBIDDEN' ? '只有管理員可以調整此設定' : err.message
    }
    else {
      transferToggleError.value = err instanceof Error ? err.message : '操作失敗'
    }
  }
  finally {
    transferToggleSaving.value = false
  }
}

async function toggleLocationTracking() {
  if (!auth.currentOrg.value) return
  locationTrackingToggleError.value = ''
  locationTrackingToggleSaving.value = true
  const target = !auth.currentOrg.value.checkin.location_tracking_enabled
  try {
    await orgSettings.update({ location_tracking_enabled: target })
    await auth.refresh()
  }
  catch (err) {
    if (err instanceof ApiError && err.code === 'STATE_LOCKED') {
      locationTrackingToggleError.value = '目前有 App 使用者在班，需先全部下班才能調整此設定'
    }
    else if (err instanceof ApiError) {
      locationTrackingToggleError.value = err.code === 'FORBIDDEN' ? '只有管理員可以調整此設定' : err.message
    }
    else {
      locationTrackingToggleError.value = err instanceof Error ? err.message : '操作失敗'
    }
  }
  finally {
    locationTrackingToggleSaving.value = false
  }
}

const COMMON_TIMEZONES = [
  'Asia/Taipei',
  'Asia/Tokyo',
  'Asia/Hong_Kong',
  'Asia/Singapore',
  'Asia/Shanghai',
  'Asia/Seoul',
  'America/Los_Angeles',
  'America/New_York',
  'Europe/London',
  'UTC',
]
const tzInput = ref('')
const tzEditing = ref(false)
const tzSaving = ref(false)
const tzError = ref('')

function startEditTz() {
  tzInput.value = auth.currentOrg.value?.timezone ?? ''
  tzEditing.value = true
  tzError.value = ''
}

function cancelEditTz() {
  tzEditing.value = false
  tzError.value = ''
}

async function saveTz() {
  tzError.value = ''
  tzSaving.value = true
  try {
    await orgSettings.update({ timezone: tzInput.value.trim() })
    await auth.refresh()
    tzEditing.value = false
  }
  catch (err) {
    if (err instanceof ApiError) {
      tzError.value = err.code === 'INVALID_TIMEZONE'
        ? '不是有效的 IANA 時區名稱（例如 Asia/Taipei、America/Los_Angeles）'
        : err.code === 'FORBIDDEN'
          ? '只有管理員可以調整時區'
          : err.message
    }
    else {
      tzError.value = err instanceof Error ? err.message : '操作失敗'
    }
  }
  finally {
    tzSaving.value = false
  }
}

const showLeaveConfirm = ref(false)
const leaving = ref(false)
const leaveError = ref('')

async function confirmLeave() {
  leaveError.value = ''
  leaving.value = true
  try {
    await auth.leaveOrg()
  }
  catch (err) {
    if (err instanceof ApiError) {
      leaveError.value = err.code === 'OWNER_PROTECTED'
        ? '組織擁有者無法離開組織，請先轉移擁有權'
        : err.message
    }
    else {
      leaveError.value = err instanceof Error ? err.message : '操作失敗'
    }
    leaving.value = false
    return
  }
  // Server force-killed this session; the user needs to log in again to reach
  // their other orgs (or land on /no-org if this was their only one).
  await navigateTo('/login')
}
</script>

<template>
  <main class="min-h-screen px-4 py-10">
    <div class="max-w-3xl mx-auto space-y-6">
      <header class="flex items-center justify-between gap-3">
        <div class="min-w-0">
          <h1 class="text-2xl font-semibold text-slate-900">
            argus admin
          </h1>
          <p
            v-if="auth.user.value"
            class="text-sm text-slate-500 truncate"
          >
            {{ auth.user.value.email }}
          </p>
        </div>
        <div class="flex shrink-0 items-center gap-2">
          <OrgSwitcher />
          <button
            type="button"
            class="rounded-md border border-slate-300 bg-white px-3 py-2 text-sm font-medium text-slate-700 hover:bg-slate-50"
            @click="onLogout"
          >
            登出
          </button>
        </div>
      </header>

      <section
        v-if="auth.currentOrg.value"
        class="rounded-xl border border-slate-200 bg-white p-6 shadow-sm"
      >
        <h2 class="text-lg font-semibold text-slate-900 mb-4">
          組織資訊
        </h2>

        <dl class="space-y-3 text-sm">
          <div class="flex items-baseline gap-3">
            <dt class="w-24 text-slate-500">
              名稱
            </dt>
            <dd class="font-medium text-slate-900">
              {{ auth.currentOrg.value.name }}
            </dd>
          </div>

          <div class="flex items-baseline gap-3">
            <dt class="w-24 text-slate-500">
              組織代碼
            </dt>
            <dd class="flex items-center gap-2">
              <code class="rounded bg-slate-100 px-2 py-1 font-mono tracking-widest text-slate-900">{{ auth.currentOrg.value.code }}</code>
              <button
                type="button"
                class="text-xs text-slate-600 hover:text-slate-900"
                @click="copyCode"
              >
                {{ copied ? '已複製' : '複製代碼' }}
              </button>
            </dd>
          </div>

          <div class="flex items-baseline gap-3">
            <dt class="w-24 text-slate-500">
              自訂代碼
            </dt>
            <dd class="flex flex-wrap items-center gap-2">
              <template v-if="!slugEditing">
                <code
                  v-if="currentSlug"
                  class="rounded bg-slate-100 px-2 py-1 font-mono text-slate-900"
                >{{ currentSlug }}</code>
                <span
                  v-else
                  class="text-slate-500 italic"
                >未設定</span>
                <template v-if="auth.isAdmin.value">
                  <button
                    type="button"
                    class="text-xs text-slate-600 hover:text-slate-900"
                    @click="startEditSlug"
                  >
                    {{ currentSlug ? '變更' : '設定' }}
                  </button>
                  <button
                    v-if="currentSlug"
                    type="button"
                    class="text-xs text-red-600 hover:text-red-800"
                    @click="showClearConfirm = true"
                  >
                    清除
                  </button>
                </template>
              </template>
              <template v-else>
                <input
                  v-model="slugInput"
                  type="text"
                  class="rounded border border-slate-300 px-2 py-1 font-mono text-sm"
                  :placeholder="slugFormatHint"
                  :disabled="slugSaving"
                  maxlength="24"
                >
                <button
                  type="button"
                  :disabled="slugSaving"
                  class="rounded-md bg-slate-900 px-3 py-1 text-xs font-medium text-white hover:bg-slate-800 disabled:opacity-60"
                  @click="saveSlug"
                >
                  {{ slugSaving ? '儲存中…' : '儲存' }}
                </button>
                <button
                  type="button"
                  :disabled="slugSaving"
                  class="rounded-md border border-slate-300 px-3 py-1 text-xs font-medium text-slate-700 hover:bg-slate-50"
                  @click="cancelEditSlug"
                >
                  取消
                </button>
              </template>
            </dd>
          </div>

          <div
            v-if="slugError && !showClearConfirm"
            class="ml-[6.5rem] text-xs text-red-600"
          >
            {{ slugError }}
            <span v-if="slugRetryAfter">（{{ formatRetryAfter(slugRetryAfter) }} 後可再變更）</span>
          </div>

          <div class="flex items-baseline gap-3">
            <dt class="w-24 text-slate-500">
              邀請連結
            </dt>
            <dd class="flex items-center gap-2 min-w-0">
              <code class="truncate rounded bg-slate-100 px-2 py-1 font-mono text-xs text-slate-700">{{ inviteUrl }}</code>
              <button
                type="button"
                class="shrink-0 text-xs text-slate-600 hover:text-slate-900"
                @click="copyInvite"
              >
                複製連結
              </button>
            </dd>
          </div>
        </dl>

        <div
          v-if="showClearConfirm"
          class="mt-4 rounded-md border border-amber-200 bg-amber-50 p-4 space-y-3"
        >
          <p class="text-sm text-amber-900">
            確定要清除自訂代碼 <code class="font-mono">{{ currentSlug }}</code>？接下來 30 天內，此代碼仍會指向你的組織但無法被其他組織使用，30 天後才會釋出。
          </p>
          <div class="flex gap-2">
            <button
              type="button"
              :disabled="slugSaving"
              class="rounded-md bg-red-600 px-3 py-2 text-sm font-medium text-white hover:bg-red-700 disabled:opacity-60"
              @click="clearSlug"
            >
              {{ slugSaving ? '清除中…' : '確認清除' }}
            </button>
            <button
              type="button"
              :disabled="slugSaving"
              class="rounded-md border border-slate-300 bg-white px-3 py-2 text-sm font-medium text-slate-700 hover:bg-slate-50"
              @click="showClearConfirm = false"
            >
              取消
            </button>
          </div>
          <p
            v-if="slugError"
            class="text-sm text-red-600"
          >
            {{ slugError }}
            <span v-if="slugRetryAfter">（{{ formatRetryAfter(slugRetryAfter) }} 後可再變更）</span>
          </p>
        </div>
      </section>

      <section
        v-if="auth.isAdmin.value"
        class="rounded-xl border border-slate-200 bg-white p-6 shadow-sm space-y-4"
      >
        <div class="flex items-start justify-between gap-4">
          <div>
            <h2 class="text-lg font-semibold text-slate-900">
              管理員工具
            </h2>
            <p class="text-sm text-slate-500">
              輪替組織代碼後，舊代碼將無法再加入組織。
            </p>
          </div>
          <div class="flex shrink-0 flex-wrap justify-end gap-2">
            <NuxtLink
              to="/members"
              class="rounded-md border border-slate-300 bg-white px-3 py-2 text-sm font-medium text-slate-700 hover:bg-slate-50"
            >
              成員管理
            </NuxtLink>
            <NuxtLink
              to="/app-users"
              class="rounded-md border border-slate-300 bg-white px-3 py-2 text-sm font-medium text-slate-700 hover:bg-slate-50"
            >
              App 使用者
            </NuxtLink>
            <NuxtLink
              to="/checkin"
              class="rounded-md border border-slate-300 bg-white px-3 py-2 text-sm font-medium text-slate-700 hover:bg-slate-50"
            >
              打卡看板
            </NuxtLink>
            <NuxtLink
              to="/cooldowns"
              class="rounded-md border border-slate-300 bg-white px-3 py-2 text-sm font-medium text-slate-700 hover:bg-slate-50"
            >
              冷卻管理
            </NuxtLink>
            <NuxtLink
              to="/admin/join-requests"
              class="relative rounded-md border border-slate-300 bg-white px-3 py-2 text-sm font-medium text-slate-700 hover:bg-slate-50"
            >
              加入申請
              <span
                v-if="pendingJoinCount > 0"
                class="ml-1 inline-flex min-w-5 items-center justify-center rounded-full bg-red-500 px-1.5 text-xs font-semibold text-white"
              >
                {{ pendingJoinCount }}
              </span>
            </NuxtLink>
          </div>
        </div>

        <div v-if="!showRotateConfirm">
          <button
            type="button"
            class="rounded-md border border-slate-300 bg-white px-3 py-2 text-sm font-medium text-slate-700 hover:bg-slate-50"
            @click="showRotateConfirm = true"
          >
            輪替組織代碼
          </button>
        </div>

        <div
          v-else
          class="rounded-md border border-amber-200 bg-amber-50 p-4 space-y-3"
        >
          <p class="text-sm text-amber-900">
            確定要輪替組織代碼嗎？舊代碼將立刻失效，已分享的邀請連結將無法再使用。
          </p>
          <div class="flex gap-2">
            <button
              type="button"
              :disabled="rotating"
              class="rounded-md bg-slate-900 px-3 py-2 text-sm font-medium text-white hover:bg-slate-800 disabled:opacity-60"
              @click="rotateCode"
            >
              {{ rotating ? '輪替中…' : '確認輪替' }}
            </button>
            <button
              type="button"
              :disabled="rotating"
              class="rounded-md border border-slate-300 bg-white px-3 py-2 text-sm font-medium text-slate-700 hover:bg-slate-50"
              @click="showRotateConfirm = false"
            >
              取消
            </button>
          </div>
          <p
            v-if="rotateError"
            class="text-sm text-red-600"
          >
            {{ rotateError }}
          </p>
        </div>
      </section>

      <section
        v-if="auth.isAdmin.value && auth.currentOrg.value"
        class="rounded-xl border border-slate-200 bg-white p-6 shadow-sm space-y-4"
      >
        <div>
          <h2 class="text-lg font-semibold text-slate-900">
            打卡設定
          </h2>
          <p class="text-sm text-slate-500">
            轉出 / 轉入功能與顯示用時區。
          </p>
        </div>

        <dl class="space-y-4 text-sm">
          <div class="flex flex-wrap items-baseline gap-3">
            <dt class="w-24 text-slate-500">
              轉出 / 轉入
            </dt>
            <dd class="flex flex-wrap items-center gap-2">
              <label class="inline-flex cursor-pointer items-center gap-2">
                <input
                  type="checkbox"
                  :checked="auth.currentOrg.value.checkin.transfer_enabled"
                  :disabled="transferToggleSaving"
                  class="h-4 w-4 rounded border-slate-300 text-slate-900"
                  @change="toggleTransfer"
                >
                <span class="text-slate-700">
                  {{ auth.currentOrg.value.checkin.transfer_enabled ? '啟用' : '停用' }}
                </span>
              </label>
              <span class="text-xs text-slate-500">
                關閉後，App 端只能上下班，無法 transfer_out / transfer_in。
              </span>
            </dd>
          </div>
          <div
            v-if="transferToggleError"
            class="ml-[6.5rem] text-xs text-red-600"
          >
            {{ transferToggleError }}
          </div>

          <div class="flex flex-wrap items-baseline gap-3">
            <dt class="w-24 text-slate-500">
              定位追蹤
            </dt>
            <dd class="flex flex-wrap items-center gap-2">
              <label class="inline-flex cursor-pointer items-center gap-2">
                <input
                  type="checkbox"
                  :checked="auth.currentOrg.value.checkin.location_tracking_enabled"
                  :disabled="locationTrackingToggleSaving"
                  class="h-4 w-4 rounded border-slate-300 text-slate-900"
                  @change="toggleLocationTracking"
                >
                <span class="text-slate-700">
                  {{ auth.currentOrg.value.checkin.location_tracking_enabled ? '啟用' : '停用' }}
                </span>
              </label>
              <span class="text-xs text-slate-500">
                關閉後，App 端不再蒐集工作期間定位軌跡。已存在的軌跡資料不受影響。
              </span>
            </dd>
          </div>
          <div
            v-if="locationTrackingToggleError"
            class="ml-[6.5rem] text-xs text-red-600"
          >
            {{ locationTrackingToggleError }}
          </div>

          <div class="flex flex-wrap items-baseline gap-3">
            <dt class="w-24 text-slate-500">
              組織時區
            </dt>
            <dd class="flex flex-wrap items-center gap-2">
              <template v-if="!tzEditing">
                <code class="rounded bg-slate-100 px-2 py-1 font-mono text-slate-900">
                  {{ auth.currentOrg.value.timezone }}
                </code>
                <button
                  type="button"
                  class="text-xs text-slate-600 hover:text-slate-900"
                  @click="startEditTz"
                >
                  變更
                </button>
              </template>
              <template v-else>
                <select
                  v-model="tzInput"
                  :disabled="tzSaving"
                  class="rounded border border-slate-300 px-2 py-1 font-mono text-sm"
                >
                  <option
                    v-for="tz in COMMON_TIMEZONES"
                    :key="tz"
                    :value="tz"
                  >
                    {{ tz }}
                  </option>
                  <option :value="tzInput">
                    {{ COMMON_TIMEZONES.includes(tzInput) ? '— 自訂 —' : tzInput || '— 自訂 —' }}
                  </option>
                </select>
                <input
                  v-model="tzInput"
                  type="text"
                  class="rounded border border-slate-300 px-2 py-1 font-mono text-sm"
                  placeholder="或自訂 IANA 名稱"
                  :disabled="tzSaving"
                >
                <button
                  type="button"
                  :disabled="tzSaving"
                  class="rounded-md bg-slate-900 px-3 py-1 text-xs font-medium text-white hover:bg-slate-800 disabled:opacity-60"
                  @click="saveTz"
                >
                  {{ tzSaving ? '儲存中…' : '儲存' }}
                </button>
                <button
                  type="button"
                  :disabled="tzSaving"
                  class="rounded-md border border-slate-300 px-3 py-1 text-xs font-medium text-slate-700 hover:bg-slate-50"
                  @click="cancelEditTz"
                >
                  取消
                </button>
              </template>
            </dd>
          </div>
          <div
            v-if="tzError"
            class="ml-[6.5rem] text-xs text-red-600"
          >
            {{ tzError }}
          </div>

          <p class="text-xs text-slate-500">
            時區僅影響 admin-web 顯示，資料庫一律存絕對時間。
          </p>
        </dl>
      </section>

      <section
        v-if="auth.currentOrg.value"
        class="rounded-xl border border-red-200 bg-white p-6 shadow-sm space-y-4"
      >
        <div>
          <h2 class="text-lg font-semibold text-red-700">
            離開組織
          </h2>
          <p class="text-sm text-slate-500">
            離開後，你會從此組織登出（其他組織的登入狀態不受影響，但需在此瀏覽器重新登入）。7 天內無法以同一 email 重新加入此組織。
          </p>
        </div>

        <div v-if="!showLeaveConfirm">
          <button
            type="button"
            :disabled="auth.isOwner.value"
            :title="auth.isOwner.value ? '組織擁有者無法離開，請先轉移擁有權' : ''"
            class="rounded-md border border-red-300 bg-white px-3 py-2 text-sm font-medium text-red-700 hover:bg-red-50 disabled:opacity-60 disabled:cursor-not-allowed"
            @click="showLeaveConfirm = true"
          >
            離開組織
          </button>
          <p
            v-if="auth.isOwner.value"
            class="mt-2 text-xs text-slate-500"
          >
            你是組織擁有者，需要先在「成員管理」轉移擁有權給另一位管理員，才能離開。
          </p>
        </div>

        <div
          v-else
          class="rounded-md border border-red-200 bg-red-50 p-4 space-y-3"
        >
          <p class="text-sm text-red-900">
            確定要離開組織？目前 session 將被結束；7 天內無法以同一 email 重新加入此組織。
          </p>
          <div class="flex gap-2">
            <button
              type="button"
              :disabled="leaving"
              class="rounded-md bg-red-600 px-3 py-2 text-sm font-medium text-white hover:bg-red-700 disabled:opacity-60"
              @click="confirmLeave"
            >
              {{ leaving ? '離開中…' : '確認離開' }}
            </button>
            <button
              type="button"
              :disabled="leaving"
              class="rounded-md border border-slate-300 bg-white px-3 py-2 text-sm font-medium text-slate-700 hover:bg-slate-50"
              @click="showLeaveConfirm = false"
            >
              取消
            </button>
          </div>
          <p
            v-if="leaveError"
            class="text-sm text-red-600"
          >
            {{ leaveError }}
          </p>
        </div>
      </section>
    </div>
  </main>
</template>
