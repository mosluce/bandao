<script setup lang="ts">
import type { ApiTokenDto, ApiTokenScope } from '~/types/api'
import { API_TOKEN_SCOPES, ApiError } from '~/types/api'

definePageMeta({ middleware: 'auth' })

const auth = useAuth()
const apiTokens = useApiTokens()

const items = ref<ApiTokenDto[]>([])
const loading = ref(true)
const loadError = ref('')

const showCreateForm = ref(false)
const newName = ref('')
const newScopes = ref<ApiTokenScope[]>([])
const creating = ref(false)
const createError = ref('')

const pendingId = ref<string | null>(null)
const actionError = ref('')

const confirmDisableId = ref<string | null>(null)
const confirmRotateId = ref<string | null>(null)
const confirmDeleteId = ref<string | null>(null)

// One-time secret modal: shown after create or rotate. Closing it drops the
// plaintext for good — the server never returns it again.
const secretModal = ref<{ token: ApiTokenDto, secret: string } | null>(null)
const secretCopied = ref(false)

async function load() {
  loadError.value = ''
  loading.value = true
  try {
    items.value = await apiTokens.list()
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
  newName.value = ''
  newScopes.value = []
  createError.value = ''
}

function cancelCreate() {
  showCreateForm.value = false
  createError.value = ''
}

async function submitCreate() {
  createError.value = ''
  if (newScopes.value.length === 0) {
    createError.value = '至少要選一個 scope'
    return
  }
  creating.value = true
  try {
    const res = await apiTokens.create({ name: newName.value.trim(), scopes: newScopes.value })
    items.value = [res.token, ...items.value]
    secretModal.value = { token: res.token, secret: res.secret }
    showCreateForm.value = false
    newName.value = ''
    newScopes.value = []
  }
  catch (err) {
    createError.value = friendlyCreateError(err)
  }
  finally {
    creating.value = false
  }
}

async function confirmRotate() {
  const id = confirmRotateId.value
  if (!id) return
  actionError.value = ''
  pendingId.value = id
  try {
    const res = await apiTokens.rotate(id)
    replaceItem(res.token)
    secretModal.value = { token: res.token, secret: res.secret }
    confirmRotateId.value = null
  }
  catch (err) {
    actionError.value = friendlyActionError(err)
  }
  finally {
    pendingId.value = null
  }
}

async function confirmDisable() {
  const id = confirmDisableId.value
  if (!id) return
  actionError.value = ''
  pendingId.value = id
  try {
    const updated = await apiTokens.disable(id)
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

async function enableToken(id: string) {
  actionError.value = ''
  pendingId.value = id
  try {
    const updated = await apiTokens.enable(id)
    replaceItem(updated)
  }
  catch (err) {
    actionError.value = friendlyActionError(err)
  }
  finally {
    pendingId.value = null
  }
}

async function confirmDelete() {
  const id = confirmDeleteId.value
  if (!id) return
  actionError.value = ''
  pendingId.value = id
  try {
    await apiTokens.remove(id)
    items.value = items.value.filter(t => t.id !== id)
    confirmDeleteId.value = null
  }
  catch (err) {
    actionError.value = friendlyActionError(err)
  }
  finally {
    pendingId.value = null
  }
}

function replaceItem(updated: ApiTokenDto) {
  const idx = items.value.findIndex(t => t.id === updated.id)
  if (idx >= 0) items.value[idx] = updated
}

async function copySecret() {
  if (!secretModal.value) return
  try {
    await navigator.clipboard.writeText(secretModal.value.secret)
    secretCopied.value = true
    setTimeout(() => { secretCopied.value = false }, 1500)
  }
  catch {
    // ignore — clipboard may be unavailable
  }
}

function dismissSecretModal() {
  secretModal.value = null
  secretCopied.value = false
}

function scopeLabel(scope: ApiTokenScope): string {
  return API_TOKEN_SCOPES.find(s => s.value === scope)?.label ?? scope
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
    case 'VALIDATION':
      return err.message
    case 'NO_ACTIVE_ORG':
      return '尚未選擇組織'
    case 'FORBIDDEN':
      return '只有管理員可以建立 API Token'
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
      return '找不到此 API Token'
    case 'FORBIDDEN':
      return '只有管理員可以執行此操作'
    case 'NO_ACTIVE_ORG':
      return '尚未選擇組織'
    default:
      return err.message
  }
}

if (auth.isAdmin.value) {
  await load()
}
else {
  await navigateTo('/')
}
</script>

<template>
  <main class="min-h-screen px-4 py-10">
    <div class="max-w-4xl mx-auto space-y-6">
      <header>
        <h1 class="text-2xl font-semibold text-slate-900">
          API Token
        </h1>
        <p class="text-sm text-slate-500 truncate">
          {{ auth.currentOrg.value?.name }} · 給外部系統（排程腳本等）呼叫班到 API 用的長效憑證
        </p>
      </header>

      <section class="rounded-xl border border-slate-200 bg-white p-6 shadow-sm">
        <div
          v-if="!showCreateForm"
          class="flex items-center justify-between"
        >
          <p class="text-sm text-slate-500">
            Token 無到期時間，僅能由管理員手動 rotate / 停用 / 刪除。建立後密鑰只顯示一次。
          </p>
          <button
            type="button"
            class="rounded-md bg-slate-900 px-3 py-2 text-sm font-medium text-white hover:bg-slate-800"
            @click="openCreate"
          >
            + 建立 API Token
          </button>
        </div>

        <form
          v-else
          class="space-y-4"
          @submit.prevent="submitCreate"
        >
          <div>
            <label
              for="newTokenName"
              class="block text-sm font-medium text-slate-700 mb-1"
            >名稱</label>
            <input
              id="newTokenName"
              v-model="newName"
              type="text"
              required
              minlength="1"
              maxlength="60"
              :disabled="creating"
              placeholder="例如：震旦雲匯出"
              class="w-full rounded-md border border-slate-300 px-3 py-2 text-sm focus:border-slate-900 focus:outline-none focus:ring-1 focus:ring-slate-900"
            >
          </div>

          <div>
            <span class="block text-sm font-medium text-slate-700 mb-1">Scope</span>
            <div class="space-y-2">
              <label
                v-for="scope in API_TOKEN_SCOPES"
                :key="scope.value"
                class="flex items-start gap-2 text-sm text-slate-700"
              >
                <input
                  v-model="newScopes"
                  type="checkbox"
                  :value="scope.value"
                  :disabled="creating"
                  class="mt-0.5"
                >
                <span>{{ scope.label }}</span>
              </label>
            </div>
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
          目前沒有 API Token。點上方「建立 API Token」開始。
        </p>

        <table
          v-else
          class="w-full text-left text-sm"
        >
          <thead class="bg-slate-50 text-xs uppercase text-slate-500">
            <tr>
              <th class="px-6 py-3 font-medium">
                名稱
              </th>
              <th class="px-6 py-3 font-medium">
                Scope
              </th>
              <th class="px-6 py-3 font-medium">
                狀態
              </th>
              <th class="px-6 py-3 font-medium">
                最後使用
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
              v-for="t in items"
              :key="t.id"
              :class="t.status === 'disabled' ? 'opacity-60' : ''"
            >
              <td class="px-6 py-3">
                <div class="font-medium text-slate-900">
                  {{ t.name }}
                </div>
                <div class="font-mono text-xs text-slate-400">
                  {{ t.token_prefix }}…
                </div>
              </td>
              <td class="px-6 py-3 text-slate-700">
                <span
                  v-for="scope in t.scopes"
                  :key="scope"
                  class="mr-1 inline-block rounded bg-slate-100 px-1.5 py-0.5 text-xs text-slate-700"
                  :title="scopeLabel(scope)"
                >{{ scope }}</span>
              </td>
              <td class="px-6 py-3">
                <span
                  v-if="t.status === 'active'"
                  class="rounded bg-green-100 px-1.5 py-0.5 text-xs text-green-800"
                >啟用</span>
                <span
                  v-else
                  class="rounded bg-slate-200 px-1.5 py-0.5 text-xs text-slate-700"
                >已停用</span>
              </td>
              <td class="px-6 py-3 text-slate-700">
                {{ formatDate(t.last_used_at) }}
              </td>
              <td class="px-6 py-3 text-slate-700">
                {{ formatDate(t.created_at) }}
              </td>
              <td class="px-6 py-3">
                <div class="flex flex-wrap gap-2">
                  <button
                    type="button"
                    :disabled="pendingId === t.id"
                    class="rounded-md border border-slate-300 bg-white px-3 py-1.5 text-xs font-medium text-slate-700 hover:bg-slate-50 disabled:opacity-60"
                    @click="confirmRotateId = t.id"
                  >
                    Rotate
                  </button>
                  <button
                    v-if="t.status === 'active'"
                    type="button"
                    :disabled="pendingId === t.id"
                    class="rounded-md border border-amber-300 bg-white px-3 py-1.5 text-xs font-medium text-amber-700 hover:bg-amber-50 disabled:opacity-60"
                    @click="confirmDisableId = t.id"
                  >
                    停用
                  </button>
                  <button
                    v-else
                    type="button"
                    :disabled="pendingId === t.id"
                    class="rounded-md border border-green-300 bg-white px-3 py-1.5 text-xs font-medium text-green-700 hover:bg-green-50 disabled:opacity-60"
                    @click="enableToken(t.id)"
                  >
                    啟用
                  </button>
                  <button
                    type="button"
                    :disabled="pendingId === t.id"
                    class="rounded-md border border-red-300 bg-white px-3 py-1.5 text-xs font-medium text-red-700 hover:bg-red-50 disabled:opacity-60"
                    @click="confirmDeleteId = t.id"
                  >
                    刪除
                  </button>
                </div>
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
        v-if="confirmRotateId"
        class="rounded-md border border-amber-200 bg-amber-50 p-4 space-y-3"
      >
        <p class="text-sm text-amber-900">
          確定要 rotate 這個 token？會產生新的密鑰，舊密鑰立即失效——任何還在用舊密鑰的外部系統會馬上斷線，需要更新成新密鑰。
        </p>
        <div class="flex gap-2">
          <button
            type="button"
            :disabled="pendingId === confirmRotateId"
            class="rounded-md bg-amber-600 px-3 py-2 text-sm font-medium text-white hover:bg-amber-700 disabled:opacity-60"
            @click="confirmRotate"
          >
            {{ pendingId === confirmRotateId ? 'Rotate 中…' : '確認 Rotate' }}
          </button>
          <button
            type="button"
            :disabled="pendingId === confirmRotateId"
            class="rounded-md border border-slate-300 bg-white px-3 py-2 text-sm font-medium text-slate-700 hover:bg-slate-50"
            @click="confirmRotateId = null"
          >
            取消
          </button>
        </div>
      </div>

      <div
        v-if="confirmDisableId"
        class="rounded-md border border-amber-200 bg-amber-50 p-4 space-y-3"
      >
        <p class="text-sm text-amber-900">
          確定要停用此 token？停用後立即無法通過驗證，但密鑰保留，之後可重新啟用恢復使用（不用重新產生密鑰）。
        </p>
        <div class="flex gap-2">
          <button
            type="button"
            :disabled="pendingId === confirmDisableId"
            class="rounded-md bg-amber-600 px-3 py-2 text-sm font-medium text-white hover:bg-amber-700 disabled:opacity-60"
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
        v-if="confirmDeleteId"
        class="rounded-md border border-red-200 bg-red-50 p-4 space-y-3"
      >
        <p class="text-sm text-red-900">
          確定要刪除此 token？此動作無法復原，之後只能重新建立一個全新的 token。
        </p>
        <div class="flex gap-2">
          <button
            type="button"
            :disabled="pendingId === confirmDeleteId"
            class="rounded-md bg-red-600 px-3 py-2 text-sm font-medium text-white hover:bg-red-700 disabled:opacity-60"
            @click="confirmDelete"
          >
            {{ pendingId === confirmDeleteId ? '刪除中…' : '確認刪除' }}
          </button>
          <button
            type="button"
            :disabled="pendingId === confirmDeleteId"
            class="rounded-md border border-slate-300 bg-white px-3 py-2 text-sm font-medium text-slate-700 hover:bg-slate-50"
            @click="confirmDeleteId = null"
          >
            取消
          </button>
        </div>
      </div>

      <div
        v-if="secretModal"
        class="fixed inset-0 z-50 flex items-center justify-center bg-slate-900/40 px-4"
      >
        <div class="w-full max-w-md rounded-xl bg-white p-6 shadow-xl space-y-4">
          <div>
            <h3 class="text-lg font-semibold text-slate-900">
              API Token 密鑰
            </h3>
            <p class="text-sm text-slate-500 mt-1">
              <strong>{{ secretModal.token.name }}</strong> 的密鑰。<br>
              關閉此視窗後將無法再次取得，請立刻複製到目標系統的設定裡。
            </p>
          </div>

          <div class="flex items-center gap-2 rounded-md bg-slate-100 p-3">
            <code class="flex-1 break-all font-mono text-sm text-slate-900">{{ secretModal.secret }}</code>
            <button
              type="button"
              class="shrink-0 rounded-md border border-slate-300 bg-white px-3 py-1 text-xs font-medium text-slate-700 hover:bg-slate-50"
              @click="copySecret"
            >
              {{ secretCopied ? '已複製' : '複製' }}
            </button>
          </div>

          <div class="flex justify-end">
            <button
              type="button"
              class="rounded-md bg-slate-900 px-4 py-2 text-sm font-medium text-white hover:bg-slate-800"
              @click="dismissSecretModal"
            >
              我知道了
            </button>
          </div>
        </div>
      </div>
    </div>
  </main>
</template>
