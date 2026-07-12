<script setup lang="ts">
import type { AppUserDto, CreateAppUserResponse } from '~/types/api'
import { ApiError } from '~/types/api'

definePageMeta({ middleware: 'auth' })

const auth = useAuth()
const appUsers = useAppUsers()

const items = ref<AppUserDto[]>([])
const loading = ref(true)
const loadError = ref('')

// External-auth mode: users come from the external DB (no create / reset here);
// the roster only shows shadow identities that have logged in at least once.
const isExternal = computed(() => auth.currentOrg.value?.auth_source === 'external_db')

const showCreateForm = ref(false)
const newUsername = ref('')
const newDisplayName = ref('')
const creating = ref(false)
const createError = ref('')

const pendingId = ref<string | null>(null)
const actionError = ref('')

const confirmDisableId = ref<string | null>(null)
const confirmResetId = ref<string | null>(null)

// One-time password modal: shown after create or reset.
const otpModal = ref<{ user: AppUserDto, password: string } | null>(null)
const otpCopied = ref(false)

async function load() {
  loadError.value = ''
  loading.value = true
  try {
    items.value = await appUsers.list()
  }
  catch (err) {
    loadError.value = err instanceof Error ? err.message : '載入失敗'
  }
  finally {
    loading.value = false
  }
}

watch(() => auth.currentOrg.value?.id, (newId, oldId) => {
  if (newId && newId !== oldId) load()
})

function openCreate() {
  showCreateForm.value = true
  newUsername.value = ''
  newDisplayName.value = ''
  createError.value = ''
}

function cancelCreate() {
  showCreateForm.value = false
  createError.value = ''
}

async function submitCreate() {
  createError.value = ''
  creating.value = true
  try {
    const res: CreateAppUserResponse = await appUsers.create({
      username: newUsername.value.trim(),
      display_name: newDisplayName.value.trim(),
    })
    items.value = [res.user, ...items.value]
    otpModal.value = { user: res.user, password: res.initial_password }
    showCreateForm.value = false
    newUsername.value = ''
    newDisplayName.value = ''
  }
  catch (err) {
    createError.value = friendlyCreateError(err)
  }
  finally {
    creating.value = false
  }
}

async function confirmDisable() {
  const id = confirmDisableId.value
  if (!id) return
  actionError.value = ''
  pendingId.value = id
  try {
    const updated = await appUsers.disable(id)
    replaceItem(updated)
    confirmDisableId.value = null
  }
  catch (err) {
    actionError.value = friendlyActionError(err)
  }
  finally {
    pendingId.value = null
  }
}

async function enableUser(id: string) {
  actionError.value = ''
  pendingId.value = id
  try {
    const updated = await appUsers.enable(id)
    replaceItem(updated)
  }
  catch (err) {
    actionError.value = friendlyActionError(err)
  }
  finally {
    pendingId.value = null
  }
}

async function confirmReset() {
  const id = confirmResetId.value
  if (!id) return
  actionError.value = ''
  pendingId.value = id
  try {
    const res = await appUsers.resetPassword(id)
    replaceItem(res.user)
    otpModal.value = { user: res.user, password: res.initial_password }
    confirmResetId.value = null
  }
  catch (err) {
    actionError.value = friendlyActionError(err)
  }
  finally {
    pendingId.value = null
  }
}

function replaceItem(updated: AppUserDto) {
  const idx = items.value.findIndex(u => u.id === updated.id)
  if (idx >= 0) items.value[idx] = updated
}

async function copyPassword() {
  if (!otpModal.value) return
  try {
    await navigator.clipboard.writeText(otpModal.value.password)
    otpCopied.value = true
    setTimeout(() => { otpCopied.value = false }, 1500)
  }
  catch {
    // ignore — clipboard may be unavailable
  }
}

function dismissOtp() {
  otpModal.value = null
  otpCopied.value = false
}

function formatDate(iso?: string): string {
  if (!iso) return '—'
  try {
    return new Date(iso).toLocaleString()
  }
  catch {
    return iso
  }
}

function friendlyCreateError(err: unknown): string {
  if (!(err instanceof ApiError)) {
    return err instanceof Error ? err.message : '建立失敗'
  }
  switch (err.code) {
    case 'USERNAME_TAKEN':
      return '此 username 在此組織已被使用'
    case 'INVALID_USERNAME_FORMAT':
      return 'username 格式錯誤（僅允許英數字、底線、句點、連字號，長度 2–32）'
    case 'VALIDATION':
      return err.message
    case 'NO_ACTIVE_ORG':
      return '尚未選擇組織'
    case 'FORBIDDEN':
      return '只有管理員可以新增 App 使用者'
    default:
      return err.message
  }
}

function friendlyActionError(err: unknown): string {
  if (!(err instanceof ApiError)) {
    return err instanceof Error ? err.message : '操作失敗'
  }
  switch (err.code) {
    case 'NOT_FOUND':
      return '找不到此 App 使用者'
    case 'FORBIDDEN':
      return '只有管理員可以執行此操作'
    case 'NO_ACTIVE_ORG':
      return '尚未選擇組織'
    default:
      return err.message
  }
}

await load()
</script>

<template>
  <main class="min-h-screen px-4 py-10">
    <div class="max-w-4xl mx-auto space-y-6">
      <header>
        <h1 class="text-2xl font-semibold text-slate-900">
          App 使用者
        </h1>
        <p class="text-sm text-slate-500 truncate">
          {{ auth.currentOrg.value?.name }} · 管理員工 / 終端使用者帳號
        </p>
      </header>

      <section
        v-if="isExternal"
        class="rounded-xl border border-slate-200 bg-white p-6 shadow-sm"
      >
        <p class="text-sm text-slate-500">
          此組織使用<strong class="text-slate-700">外部資料庫</strong>驗證，使用者由外部系統管理。
          帳號與密碼在
          <NuxtLink to="/settings/auth" class="text-slate-900 underline">
            驗證來源設定
          </NuxtLink>
          調整；此處僅顯示曾登入過的使用者，可停用以在本地封鎖登入。
        </p>
      </section>

      <section
        v-else-if="!auth.isAdmin.value"
        class="rounded-xl border border-slate-200 bg-white p-6 shadow-sm"
      >
        <p class="text-sm text-slate-500">
          App 使用者由管理員建立與管理，無自助註冊。
        </p>
      </section>

      <section
        v-else
        class="rounded-xl border border-slate-200 bg-white p-6 shadow-sm"
      >
        <div
          v-if="!showCreateForm"
          class="flex items-center justify-between"
        >
          <p class="text-sm text-slate-500">
            App 使用者由管理員建立，無自助註冊。建立後系統會產生一次性初始密碼。
          </p>
          <button
            type="button"
            class="rounded-md bg-slate-900 px-3 py-2 text-sm font-medium text-white hover:bg-slate-800"
            @click="openCreate"
          >
            + 新增 App 使用者
          </button>
        </div>

        <form
          v-else
          class="space-y-4"
          @submit.prevent="submitCreate"
        >
          <div>
            <label
              for="newUsername"
              class="block text-sm font-medium text-slate-700 mb-1"
            >Username</label>
            <input
              id="newUsername"
              v-model="newUsername"
              type="text"
              required
              minlength="2"
              maxlength="32"
              spellcheck="false"
              autocapitalize="none"
              :disabled="creating"
              class="w-full rounded-md border border-slate-300 px-3 py-2 font-mono text-sm focus:border-slate-900 focus:outline-none focus:ring-1 focus:ring-slate-900"
            >
            <p class="text-xs text-slate-500 mt-1">
              英數字、底線、句點、連字號，2–32 字元；同組織內唯一。
            </p>
          </div>

          <div>
            <label
              for="newDisplayName"
              class="block text-sm font-medium text-slate-700 mb-1"
            >顯示名稱</label>
            <input
              id="newDisplayName"
              v-model="newDisplayName"
              type="text"
              required
              minlength="1"
              maxlength="60"
              :disabled="creating"
              class="w-full rounded-md border border-slate-300 px-3 py-2 text-sm focus:border-slate-900 focus:outline-none focus:ring-1 focus:ring-slate-900"
            >
          </div>

          <p
            v-if="createError"
            class="text-sm text-red-600"
          >
            {{ createError }}
          </p>

          <div class="flex gap-2">
            <button
              type="submit"
              :disabled="creating"
              class="rounded-md bg-slate-900 px-3 py-2 text-sm font-medium text-white hover:bg-slate-800 disabled:opacity-60"
            >
              {{ creating ? '建立中…' : '建立' }}
            </button>
            <button
              type="button"
              :disabled="creating"
              class="rounded-md border border-slate-300 bg-white px-3 py-2 text-sm font-medium text-slate-700 hover:bg-slate-50"
              @click="cancelCreate"
            >
              取消
            </button>
          </div>
        </form>
      </section>

      <section class="rounded-xl border border-slate-200 bg-white shadow-sm">
        <p
          v-if="loadError"
          class="px-6 py-4 text-sm text-red-600"
        >
          {{ loadError }}
        </p>

        <p
          v-else-if="loading"
          class="px-6 py-4 text-sm text-slate-500"
        >
          載入中…
        </p>

        <p
          v-else-if="items.length === 0"
          class="px-6 py-8 text-center text-sm text-slate-500"
        >
          {{ isExternal
            ? '目前沒有使用者。使用者需先用外部帳號登入一次，才會出現在此。'
            : auth.isAdmin.value
              ? '目前沒有 App 使用者。點上方「新增 App 使用者」開始。'
              : '目前沒有 App 使用者。' }}
        </p>

        <table
          v-else
          class="w-full text-left text-sm"
        >
          <thead class="bg-slate-50 text-xs uppercase text-slate-500">
            <tr>
              <th class="px-6 py-3 font-medium">
                {{ isExternal ? '唯一識別' : 'Username' }}
              </th>
              <th class="px-6 py-3 font-medium">
                顯示名稱
              </th>
              <th class="px-6 py-3 font-medium">
                狀態
              </th>
              <th class="px-6 py-3 font-medium">
                上次登入
              </th>
              <th class="px-6 py-3 font-medium">
                建立時間
              </th>
              <th class="px-6 py-3 font-medium">
                操作
              </th>
            </tr>
          </thead>
          <tbody class="divide-y divide-slate-200">
            <tr
              v-for="u in items"
              :key="u.id"
              :class="u.status === 'disabled' ? 'opacity-60' : ''"
            >
              <td class="px-6 py-3 font-mono font-medium text-slate-900">
                {{ u.username ?? u.external_key }}
              </td>
              <td class="px-6 py-3 text-slate-700">
                {{ u.display_name }}
              </td>
              <td class="px-6 py-3">
                <span
                  v-if="u.status === 'active'"
                  class="rounded bg-green-100 px-1.5 py-0.5 text-xs text-green-800"
                >啟用</span>
                <span
                  v-else
                  class="rounded bg-slate-200 px-1.5 py-0.5 text-xs text-slate-700"
                >已停用</span>
                <span
                  v-if="u.needs_password_change"
                  class="ml-1 rounded bg-amber-100 px-1.5 py-0.5 text-xs text-amber-800"
                  title="尚未變更初始密碼"
                >待改密碼</span>
              </td>
              <td class="px-6 py-3 text-slate-700">
                {{ formatDate(u.last_login_at) }}
              </td>
              <td class="px-6 py-3 text-slate-700">
                {{ formatDate(u.created_at) }}
              </td>
              <td class="px-6 py-3">
                <div
                  v-if="auth.isAdmin.value"
                  class="flex flex-wrap gap-2"
                >
                  <button
                    v-if="!isExternal"
                    type="button"
                    :disabled="pendingId === u.id"
                    class="rounded-md border border-slate-300 bg-white px-3 py-1.5 text-xs font-medium text-slate-700 hover:bg-slate-50 disabled:opacity-60"
                    @click="confirmResetId = u.id"
                  >
                    重設密碼
                  </button>
                  <button
                    v-if="u.status === 'active'"
                    type="button"
                    :disabled="pendingId === u.id"
                    class="rounded-md border border-red-300 bg-white px-3 py-1.5 text-xs font-medium text-red-700 hover:bg-red-50 disabled:opacity-60"
                    @click="confirmDisableId = u.id"
                  >
                    停用
                  </button>
                  <button
                    v-else
                    type="button"
                    :disabled="pendingId === u.id"
                    class="rounded-md border border-green-300 bg-white px-3 py-1.5 text-xs font-medium text-green-700 hover:bg-green-50 disabled:opacity-60"
                    @click="enableUser(u.id)"
                  >
                    啟用
                  </button>
                </div>
                <span
                  v-else
                  class="text-xs text-slate-400"
                >—</span>
              </td>
            </tr>
          </tbody>
        </table>
      </section>

      <p
        v-if="actionError"
        class="text-sm text-red-600"
      >
        {{ actionError }}
      </p>

      <div
        v-if="confirmDisableId"
        class="rounded-md border border-red-200 bg-red-50 p-4 space-y-3"
      >
        <p class="text-sm text-red-900">
          確定要停用此 App 使用者？停用後該帳號的所有登入 session 會被立刻斷線、無法再登入；密碼保留，啟用後沿用舊密碼。
        </p>
        <div class="flex gap-2">
          <button
            type="button"
            :disabled="pendingId === confirmDisableId"
            class="rounded-md bg-red-600 px-3 py-2 text-sm font-medium text-white hover:bg-red-700 disabled:opacity-60"
            @click="confirmDisable"
          >
            {{ pendingId === confirmDisableId ? '停用中…' : '確認停用' }}
          </button>
          <button
            type="button"
            :disabled="pendingId === confirmDisableId"
            class="rounded-md border border-slate-300 bg-white px-3 py-2 text-sm font-medium text-slate-700 hover:bg-slate-50"
            @click="confirmDisableId = null"
          >
            取消
          </button>
        </div>
      </div>

      <div
        v-if="confirmResetId"
        class="rounded-md border border-amber-200 bg-amber-50 p-4 space-y-3"
      >
        <p class="text-sm text-amber-900">
          確定要重設密碼？將產生新的一次性初始密碼，舊的所有 session 會被斷線；使用者下次登入時將被強制改密碼。
        </p>
        <div class="flex gap-2">
          <button
            type="button"
            :disabled="pendingId === confirmResetId"
            class="rounded-md bg-amber-600 px-3 py-2 text-sm font-medium text-white hover:bg-amber-700 disabled:opacity-60"
            @click="confirmReset"
          >
            {{ pendingId === confirmResetId ? '重設中…' : '確認重設' }}
          </button>
          <button
            type="button"
            :disabled="pendingId === confirmResetId"
            class="rounded-md border border-slate-300 bg-white px-3 py-2 text-sm font-medium text-slate-700 hover:bg-slate-50"
            @click="confirmResetId = null"
          >
            取消
          </button>
        </div>
      </div>

      <div
        v-if="otpModal"
        class="fixed inset-0 z-50 flex items-center justify-center bg-slate-900/40 px-4"
      >
        <div class="w-full max-w-md rounded-xl bg-white p-6 shadow-xl space-y-4">
          <div>
            <h3 class="text-lg font-semibold text-slate-900">
              一次性初始密碼
            </h3>
            <p class="text-sm text-slate-500 mt-1">
              <strong>{{ otpModal.user.username }}</strong>（{{ otpModal.user.display_name }}）的初始密碼。<br>
              關閉此視窗後將無法再次取得，請複製給使用者。使用者首次登入後必須變更密碼。
            </p>
          </div>

          <div class="flex items-center gap-2 rounded-md bg-slate-100 p-3">
            <code class="flex-1 break-all font-mono text-base tracking-wider text-slate-900">{{ otpModal.password }}</code>
            <button
              type="button"
              class="shrink-0 rounded-md border border-slate-300 bg-white px-3 py-1 text-xs font-medium text-slate-700 hover:bg-slate-50"
              @click="copyPassword"
            >
              {{ otpCopied ? '已複製' : '複製' }}
            </button>
          </div>

          <div class="flex justify-end">
            <button
              type="button"
              class="rounded-md bg-slate-900 px-4 py-2 text-sm font-medium text-white hover:bg-slate-800"
              @click="dismissOtp"
            >
              我知道了
            </button>
          </div>
        </div>
      </div>
    </div>
  </main>
</template>
