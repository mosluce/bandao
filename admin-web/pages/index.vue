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

const currentSlug = computed(() => auth.me.value?.org.slug ?? null)

const inviteIdentifier = computed(() => {
  const me = auth.me.value
  if (!me) return ''
  return me.org.slug || me.org.code
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
    const d = new Date(iso)
    return d.toLocaleString()
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
  if (!auth.me.value) return
  try {
    await navigator.clipboard.writeText(auth.me.value.org.code)
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

const showLeaveConfirm = ref(false)
const leaving = ref(false)
const leaveError = ref('')

const isOwner = computed(() => {
  const me = auth.me.value
  return !!me && me.user.id === me.org.owner_id
})

async function confirmLeave() {
  leaveError.value = ''
  leaving.value = true
  try {
    await api('/me/leave', { method: 'POST' })
  }
  catch (err) {
    if (err instanceof ApiError) {
      leaveError.value = err.code === 'OWNER_PROTECTED'
        ? '組織擁有者無法離開組織'
        : err.message
    }
    else {
      leaveError.value = err instanceof Error ? err.message : '操作失敗'
    }
    leaving.value = false
    return
  }
  // Server cleared the cookie; refresh will hit 401 and reset local auth state.
  try {
    await auth.refresh()
  }
  catch {
    // ignore
  }
  await navigateTo('/login')
}
</script>

<template>
  <main class="min-h-screen px-4 py-10">
    <div class="max-w-3xl mx-auto space-y-6">
      <header class="flex items-center justify-between">
        <div>
          <h1 class="text-2xl font-semibold text-slate-900">
            argus admin
          </h1>
          <p
            v-if="auth.me.value"
            class="text-sm text-slate-500"
          >
            {{ auth.me.value.user.email }} · {{ auth.me.value.role === 'admin' ? '管理員' : '成員' }}
          </p>
        </div>
        <button
          type="button"
          class="rounded-md border border-slate-300 bg-white px-3 py-2 text-sm font-medium text-slate-700 hover:bg-slate-50"
          @click="onLogout"
        >
          登出
        </button>
      </header>

      <section
        v-if="auth.me.value"
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
              {{ auth.me.value.org.name }}
            </dd>
          </div>

          <div class="flex items-baseline gap-3">
            <dt class="w-24 text-slate-500">
              組織代碼
            </dt>
            <dd class="flex items-center gap-2">
              <code class="rounded bg-slate-100 px-2 py-1 font-mono tracking-widest text-slate-900">{{ auth.me.value.org.code }}</code>
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
          <div class="flex shrink-0 gap-2">
            <NuxtLink
              to="/members"
              class="rounded-md border border-slate-300 bg-white px-3 py-2 text-sm font-medium text-slate-700 hover:bg-slate-50"
            >
              成員管理
            </NuxtLink>
            <NuxtLink
              to="/cooldowns"
              class="rounded-md border border-slate-300 bg-white px-3 py-2 text-sm font-medium text-slate-700 hover:bg-slate-50"
            >
              冷卻管理
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
        v-if="auth.me.value"
        class="rounded-xl border border-red-200 bg-white p-6 shadow-sm space-y-4"
      >
        <div>
          <h2 class="text-lg font-semibold text-red-700">
            危險區
          </h2>
          <p class="text-sm text-slate-500">
            離開組織後，將刪除你的帳號並登出。7 天內無法以同一 email 重新加入此組織。
          </p>
        </div>

        <div v-if="!showLeaveConfirm">
          <button
            type="button"
            :disabled="isOwner"
            :title="isOwner ? '組織擁有者無法離開組織' : ''"
            class="rounded-md border border-red-300 bg-white px-3 py-2 text-sm font-medium text-red-700 hover:bg-red-50 disabled:opacity-60 disabled:cursor-not-allowed"
            @click="showLeaveConfirm = true"
          >
            離開組織
          </button>
          <p
            v-if="isOwner"
            class="mt-2 text-xs text-slate-500"
          >
            你是組織擁有者，目前無法離開。需要轉移擁有權或刪除組織才能離開。
          </p>
        </div>

        <div
          v-else
          class="rounded-md border border-red-200 bg-red-50 p-4 space-y-3"
        >
          <p class="text-sm text-red-900">
            確定要離開組織？此操作無法復原；7 天內無法以同一 email 重新加入。
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
