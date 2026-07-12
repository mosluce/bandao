<script setup lang="ts">
import type { DashboardUserDto, Role } from '~/types/api'
import { ApiError } from '~/types/api'

definePageMeta({ middleware: 'auth' })

const auth = useAuth()
const api = useApi()

const users = ref<DashboardUserDto[]>([])
const loading = ref(true)
const loadError = ref('')
const pendingId = ref<string | null>(null)
const actionError = ref('')

const removeTarget = ref<DashboardUserDto | null>(null)

// Owner-transfer state per row.
const transferTargetId = ref<string | null>(null)
const transferPassword = ref('')
const transferring = ref(false)
const transferError = ref('')

const ownerId = computed(() => auth.currentOrg.value?.owner_id ?? null)
const myId = computed(() => auth.user.value?.id ?? null)

async function load() {
  loadError.value = ''
  loading.value = true
  try {
    users.value = await api<DashboardUserDto[]>('/dashboard-users', { method: 'GET' })
  }
  catch (err) {
    loadError.value = err instanceof Error ? err.message : '載入失敗'
  }
  finally {
    loading.value = false
  }
}

// Refetch when the active org changes (e.g. user switches via OrgSwitcher).
watch(() => auth.currentOrg.value?.id, (newId, oldId) => {
  if (newId && newId !== oldId) load()
})

async function changeRole(user: DashboardUserDto, target: Role) {
  if (user.role === target) return
  actionError.value = ''
  pendingId.value = user.id
  try {
    const updated = await api<DashboardUserDto>(`/dashboard-users/${user.id}/role`, {
      method: 'PATCH',
      body: { role: target },
    })
    const idx = users.value.findIndex(u => u.id === updated.id)
    if (idx >= 0) users.value[idx] = updated
    if (auth.user.value && updated.id === auth.user.value.id) {
      await auth.refresh()
      if (!auth.isAdmin.value) {
        await navigateTo('/')
      }
    }
  }
  catch (err) {
    if (err instanceof ApiError) {
      actionError.value = friendlyError(err)
    }
    else {
      actionError.value = err instanceof Error ? err.message : '操作失敗'
    }
  }
  finally {
    pendingId.value = null
  }
}

async function confirmRemove() {
  const target = removeTarget.value
  if (!target) return
  actionError.value = ''
  pendingId.value = target.id
  try {
    await api(`/dashboard-users/${target.id}`, { method: 'DELETE' })
    users.value = users.value.filter(u => u.id !== target.id)
    removeTarget.value = null
  }
  catch (err) {
    if (err instanceof ApiError) {
      actionError.value = friendlyError(err)
    }
    else {
      actionError.value = err instanceof Error ? err.message : '操作失敗'
    }
  }
  finally {
    pendingId.value = null
  }
}

function startTransfer(user: DashboardUserDto) {
  transferTargetId.value = user.id
  transferPassword.value = ''
  transferError.value = ''
}

function cancelTransfer() {
  transferTargetId.value = null
  transferPassword.value = ''
  transferError.value = ''
}

async function confirmTransfer() {
  const targetId = transferTargetId.value
  if (!targetId) return
  transferError.value = ''
  transferring.value = true
  try {
    await auth.transferOwnership(targetId, transferPassword.value)
    await load()
    cancelTransfer()
  }
  catch (err) {
    if (err instanceof ApiError) {
      transferError.value = friendlyTransferError(err)
    }
    else {
      transferError.value = err instanceof Error ? err.message : '轉移失敗'
    }
  }
  finally {
    transferring.value = false
  }
}

function friendlyError(err: ApiError): string {
  switch (err.code) {
    case 'OWNER_PROTECTED':
      return '無法對組織擁有者執行此操作'
    case 'NOT_FOUND':
      return '找不到此成員'
    case 'FORBIDDEN':
      return '只有管理員可以執行此操作'
    case 'NO_ACTIVE_ORG':
      return '尚未選擇組織'
    default:
      return err.message
  }
}

function friendlyTransferError(err: ApiError): string {
  switch (err.code) {
    case 'INVALID_PASSWORD':
      return '密碼不正確'
    case 'INVALID_TARGET':
      return '目標必須是同組織的管理員'
    case 'SAME_OWNER':
      return '不能轉移給自己'
    case 'FORBIDDEN':
      return '只有擁有者可以轉移擁有權'
    default:
      return err.message
  }
}

await load()
</script>

<template>
  <main class="min-h-screen px-4 py-10">
    <div class="max-w-3xl mx-auto space-y-6">
      <header>
        <h1 class="text-2xl font-semibold text-slate-900">
          成員管理
        </h1>
        <p class="text-sm text-slate-500 truncate">
          {{ auth.currentOrg.value?.name }} · {{ auth.isAdmin.value ? '升降級組織內的 dashboard 帳號' : '檢視組織內的 dashboard 帳號（唯讀）' }}
        </p>
      </header>

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

        <ul
          v-else
          class="divide-y divide-slate-200"
        >
          <li
            v-for="user in users"
            :key="user.id"
            class="px-6 py-4"
          >
            <div class="flex items-center justify-between gap-4">
              <div class="min-w-0">
                <p class="truncate font-medium text-slate-900">
                  {{ user.email }}
                </p>
                <p class="text-xs text-slate-500">
                  {{ user.role === 'admin' ? '管理員' : '成員' }}
                  <span
                    v-if="user.id === ownerId"
                    class="ml-1 rounded bg-amber-100 px-1.5 py-0.5 text-amber-800"
                  >擁有者</span>
                  <span
                    v-if="user.id === myId"
                    class="ml-1 text-slate-400"
                  >（你）</span>
                </p>
              </div>

              <div
                v-if="auth.isAdmin.value"
                class="flex shrink-0 flex-wrap justify-end gap-2"
              >
                <button
                  v-if="user.role === 'member'"
                  type="button"
                  :disabled="pendingId === user.id"
                  class="rounded-md bg-slate-900 px-3 py-1.5 text-xs font-medium text-white hover:bg-slate-800 disabled:opacity-60"
                  @click="changeRole(user, 'admin')"
                >
                  {{ pendingId === user.id ? '處理中…' : '升為管理員' }}
                </button>
                <button
                  v-else-if="user.id !== ownerId"
                  type="button"
                  :disabled="pendingId === user.id"
                  class="rounded-md border border-slate-300 bg-white px-3 py-1.5 text-xs font-medium text-slate-700 hover:bg-slate-50 disabled:opacity-60"
                  @click="changeRole(user, 'member')"
                >
                  {{ pendingId === user.id ? '處理中…' : '降為成員' }}
                </button>
                <button
                  v-if="auth.isOwner.value && user.role === 'admin' && user.id !== myId && user.id !== ownerId"
                  type="button"
                  :disabled="transferring || transferTargetId === user.id"
                  class="rounded-md border border-amber-300 bg-white px-3 py-1.5 text-xs font-medium text-amber-700 hover:bg-amber-50 disabled:opacity-60"
                  @click="startTransfer(user)"
                >
                  轉移擁有權
                </button>
                <button
                  v-if="user.id !== ownerId && user.id !== myId"
                  type="button"
                  :disabled="pendingId === user.id"
                  class="rounded-md border border-red-300 bg-white px-3 py-1.5 text-xs font-medium text-red-700 hover:bg-red-50 disabled:opacity-60"
                  @click="removeTarget = user"
                >
                  移除
                </button>
              </div>
            </div>

            <div
              v-if="transferTargetId === user.id"
              class="mt-3 rounded-md border border-amber-200 bg-amber-50 p-4 space-y-3"
            >
              <p class="text-sm text-amber-900">
                確認將 <strong>{{ user.email }}</strong> 設為新擁有者。轉移後你會降為一般管理員（仍可降級或被移除）。請輸入你的密碼確認。
              </p>
              <input
                v-model="transferPassword"
                type="password"
                autocomplete="current-password"
                placeholder="目前密碼"
                :disabled="transferring"
                class="w-full rounded-md border border-amber-300 px-3 py-2 text-sm focus:border-amber-600 focus:outline-none focus:ring-1 focus:ring-amber-600"
              >
              <div class="flex gap-2">
                <button
                  type="button"
                  :disabled="transferring || !transferPassword"
                  class="rounded-md bg-amber-600 px-3 py-2 text-sm font-medium text-white hover:bg-amber-700 disabled:opacity-60"
                  @click="confirmTransfer"
                >
                  {{ transferring ? '轉移中…' : '確認轉移' }}
                </button>
                <button
                  type="button"
                  :disabled="transferring"
                  class="rounded-md border border-slate-300 bg-white px-3 py-2 text-sm font-medium text-slate-700 hover:bg-slate-50"
                  @click="cancelTransfer"
                >
                  取消
                </button>
              </div>
              <p
                v-if="transferError"
                class="text-sm text-red-600"
              >
                {{ transferError }}
              </p>
            </div>
          </li>
        </ul>
      </section>

      <p
        v-if="actionError"
        class="text-sm text-red-600"
      >
        {{ actionError }}
      </p>

      <div
        v-if="removeTarget"
        class="rounded-md border border-red-200 bg-red-50 p-4 space-y-3"
      >
        <p class="text-sm text-red-900">
          確定要將
          <strong>{{ removeTarget.email }}</strong>
          從此組織移除？此操作只刪除該成員在「{{ auth.currentOrg.value?.name }}」的成員身份；他們的帳號與其他組織的身份不受影響。7 天內無法以同一 email 重新加入此組織。
        </p>
        <div class="flex gap-2">
          <button
            type="button"
            :disabled="pendingId === removeTarget.id"
            class="rounded-md bg-red-600 px-3 py-2 text-sm font-medium text-white hover:bg-red-700 disabled:opacity-60"
            @click="confirmRemove"
          >
            {{ pendingId === removeTarget.id ? '移除中…' : '確認移除' }}
          </button>
          <button
            type="button"
            :disabled="pendingId === removeTarget.id"
            class="rounded-md border border-slate-300 bg-white px-3 py-2 text-sm font-medium text-slate-700 hover:bg-slate-50"
            @click="removeTarget = null"
          >
            取消
          </button>
        </div>
      </div>
    </div>
  </main>
</template>
