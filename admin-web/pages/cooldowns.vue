<script setup lang="ts">
import type { CooldownDto } from '~/types/api'
import { ApiError } from '~/types/api'

definePageMeta({ middleware: 'auth' })

const auth = useAuth()
const api = useApi()
const router = useRouter()

const items = ref<CooldownDto[]>([])
const loading = ref(true)
const loadError = ref('')
const pendingEmail = ref<string | null>(null)
const actionError = ref('')

async function load() {
  loadError.value = ''
  loading.value = true
  try {
    items.value = await api<CooldownDto[]>('/dashboard-users/cooldowns', { method: 'GET' })
  }
  catch (err) {
    loadError.value = err instanceof Error ? err.message : '載入失敗'
  }
  finally {
    loading.value = false
  }
}

async function release(email: string) {
  actionError.value = ''
  pendingEmail.value = email
  try {
    await api(`/dashboard-users/cooldowns/${encodeURIComponent(email)}`, { method: 'DELETE' })
    items.value = items.value.filter(i => i.email !== email)
  }
  catch (err) {
    if (err instanceof ApiError) {
      actionError.value = err.code === 'FORBIDDEN' ? '只有管理員可以執行此操作' : err.message
    }
    else {
      actionError.value = err instanceof Error ? err.message : '操作失敗'
    }
  }
  finally {
    pendingEmail.value = null
  }
}

function formatAbsolute(iso: string | null): string {
  if (!iso) return ''
  try {
    return new Date(iso).toLocaleString()
  }
  catch {
    return iso
  }
}

function formatRelative(iso: string | null): string {
  if (!iso) return ''
  try {
    const target = new Date(iso).getTime()
    const now = Date.now()
    const diff = target - now
    if (diff <= 0) return '已過期'
    const days = Math.floor(diff / (1000 * 60 * 60 * 24))
    const hours = Math.floor((diff / (1000 * 60 * 60)) % 24)
    if (days > 0) return `剩餘 ${days} 天 ${hours} 小時`
    const minutes = Math.floor((diff / (1000 * 60)) % 60)
    if (hours > 0) return `剩餘 ${hours} 小時 ${minutes} 分鐘`
    return `剩餘 ${minutes} 分鐘`
  }
  catch {
    return ''
  }
}

function kindLabel(kind: CooldownDto['removal_kind']): string {
  return kind === 'kicked' ? '被移除' : '自離'
}

// Refetch when the active org changes (e.g. user switches via OrgSwitcher).
watch(() => auth.currentOrg.value?.id, (newId, oldId) => {
  if (newId && newId !== oldId) load()
})

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
      <header class="flex items-center justify-between gap-3">
        <div class="min-w-0">
          <h1 class="text-2xl font-semibold text-slate-900">
            冷卻管理
          </h1>
          <p class="text-sm text-slate-500 truncate">
            {{ auth.currentOrg.value?.name }} · 列出 7 天內被移除或自離的 email 與其冷卻到期時間
          </p>
        </div>
        <div class="flex shrink-0 items-center gap-2">
          <OrgSwitcher />
          <NuxtLink
            to="/members"
            class="rounded-md border border-slate-300 bg-white px-3 py-2 text-sm font-medium text-slate-700 hover:bg-slate-50"
          >
            成員管理
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

        <p
          v-else-if="items.length === 0"
          class="px-6 py-8 text-center text-sm text-slate-500"
        >
          目前沒有冷卻中的 email。
        </p>

        <table
          v-else
          class="w-full text-left text-sm"
        >
          <thead class="bg-slate-50 text-xs uppercase text-slate-500">
            <tr>
              <th class="px-6 py-3 font-medium">
                Email
              </th>
              <th class="px-6 py-3 font-medium">
                類型
              </th>
              <th class="px-6 py-3 font-medium">
                移除時間
              </th>
              <th class="px-6 py-3 font-medium">
                冷卻到期
              </th>
              <th class="px-6 py-3 font-medium">
                操作
              </th>
            </tr>
          </thead>
          <tbody class="divide-y divide-slate-200">
            <tr
              v-for="item in items"
              :key="item.email"
            >
              <td class="px-6 py-3 font-medium text-slate-900">
                {{ item.email }}
              </td>
              <td class="px-6 py-3 text-slate-700">
                {{ kindLabel(item.removal_kind) }}
              </td>
              <td class="px-6 py-3 text-slate-700">
                {{ formatAbsolute(item.removed_at) }}
              </td>
              <td class="px-6 py-3 text-slate-700">
                <div>{{ formatAbsolute(item.cooldown_until) }}</div>
                <div class="text-xs text-slate-500">
                  {{ formatRelative(item.cooldown_until) }}
                </div>
              </td>
              <td class="px-6 py-3">
                <button
                  type="button"
                  :disabled="pendingEmail === item.email"
                  class="rounded-md border border-slate-300 bg-white px-3 py-1.5 text-xs font-medium text-slate-700 hover:bg-slate-50 disabled:opacity-60"
                  @click="release(item.email)"
                >
                  {{ pendingEmail === item.email ? '處理中…' : '釋放冷卻' }}
                </button>
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
    </div>
  </main>
</template>
