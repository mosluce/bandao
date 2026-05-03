<script setup lang="ts">
import type { DashboardUserDto, Role } from '~/types/api'
import { ApiError } from '~/types/api'

definePageMeta({ middleware: 'auth' })

const auth = useAuth()
const api = useApi()
const router = useRouter()

const users = ref<DashboardUserDto[]>([])
const loading = ref(true)
const loadError = ref('')
const pendingId = ref<string | null>(null)
const actionError = ref('')

const removeTarget = ref<DashboardUserDto | null>(null)

const ownerId = computed(() => auth.me.value?.org.owner_id ?? null)
const myId = computed(() => auth.me.value?.user.id ?? null)

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
    if (auth.me.value && updated.id === auth.me.value.user.id) {
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

function friendlyError(err: ApiError): string {
  switch (err.code) {
    case 'OWNER_PROTECTED':
      return '無法對組織擁有者執行此操作'
    case 'NOT_FOUND':
      return '找不到此成員'
    case 'FORBIDDEN':
      return '只有管理員可以執行此操作'
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
    <div class="max-w-3xl mx-auto space-y-6">
      <header class="flex items-center justify-between">
        <div>
          <h1 class="text-2xl font-semibold text-slate-900">
            成員管理
          </h1>
          <p class="text-sm text-slate-500">
            升降級組織內的 dashboard 帳號
          </p>
        </div>
        <div class="flex gap-2">
          <NuxtLink
            to="/cooldowns"
            class="rounded-md border border-slate-300 bg-white px-3 py-2 text-sm font-medium text-slate-700 hover:bg-slate-50"
          >
            冷卻管理
          </NuxtLink>
          <button
            type="button"
            class="rounded-md border border-slate-300 bg-white px-3 py-2 text-sm font-medium text-slate-700 hover:bg-slate-50"
            @click="router.push('/')"
          >
            回首頁
          </button>
        </div>
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
            class="flex items-center justify-between gap-4 px-6 py-4"
          >
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

            <div class="flex shrink-0 gap-2">
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
                v-if="user.id !== ownerId && user.id !== myId"
                type="button"
                :disabled="pendingId === user.id"
                class="rounded-md border border-red-300 bg-white px-3 py-1.5 text-xs font-medium text-red-700 hover:bg-red-50 disabled:opacity-60"
                @click="removeTarget = user"
              >
                移除
              </button>
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
          從組織中移除？該帳號將被刪除，且 7 天內無法以同一 email 重新加入。
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
