<script setup lang="ts">
import type { JoinRequestDto } from '~/types/api'

definePageMeta({ middleware: 'auth' })

const auth = useAuth()
const joinRequests = useJoinRequests()

const myRequests = ref<JoinRequestDto[]>([])
const loadingRequests = ref(false)
const requestError = ref('')

async function loadMyRequests() {
  loadingRequests.value = true
  requestError.value = ''
  try {
    myRequests.value = await joinRequests.listMine()
  }
  catch (err) {
    requestError.value = err instanceof Error ? err.message : '載入申請失敗'
  }
  finally {
    loadingRequests.value = false
  }
}

async function onCancel(id: string) {
  try {
    await joinRequests.cancel(id)
    await loadMyRequests()
  }
  catch (err) {
    requestError.value = err instanceof Error ? err.message : '取消失敗'
  }
}

async function onLogout() {
  await auth.logout()
  await navigateTo('/login')
}

async function afterAction() {
  // After a fresh join_request the page itself stays — refresh the
  // pending list so the new entry shows up. The user becomes a real
  // member only when admin approves.
  await loadMyRequests()
}

onMounted(() => loadMyRequests())

const statusLabel: Record<string, string> = {
  pending: '審核中',
  approved: '已批准',
  rejected: '已拒絕',
  cancelled: '已取消',
}
</script>

<template>
  <main class="min-h-screen px-4 py-10">
    <div class="max-w-2xl mx-auto space-y-6">
      <header class="flex items-center justify-between">
        <div>
          <h1 class="text-2xl font-semibold text-slate-900">
            班到 admin
          </h1>
          <p
            v-if="auth.user.value"
            class="text-sm text-slate-500"
          >
            {{ auth.user.value.email }}
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

      <section class="rounded-xl border border-slate-200 bg-white p-8 shadow-sm">
        <h2 class="text-lg font-semibold text-slate-900">
          你目前不屬於任何組織
        </h2>
        <p class="mt-1 text-sm text-slate-500">
          建立一個新組織當擁有者，或用組織代碼加入既有組織。加入既有組織需經管理員審核。
        </p>
      </section>

      <section
        v-if="myRequests.length > 0 || loadingRequests"
        class="rounded-xl border border-slate-200 bg-white p-6 shadow-sm space-y-3"
      >
        <h3 class="text-base font-semibold text-slate-900">
          我的申請紀錄
        </h3>
        <p
          v-if="requestError"
          class="text-sm text-red-600"
        >
          {{ requestError }}
        </p>
        <ul class="space-y-2">
          <li
            v-for="r in myRequests"
            :key="r.id"
            class="rounded-md border border-slate-200 p-3 space-y-1"
          >
            <div class="flex items-baseline justify-between gap-2">
              <span class="font-medium text-slate-900">
                {{ r.org.name }}
              </span>
              <span class="text-xs text-slate-500">
                {{ statusLabel[r.status] || r.status }} · {{ r.requested_at }}
              </span>
            </div>
            <p
              v-if="r.application_message"
              class="text-xs text-slate-600"
            >
              附訊息：{{ r.application_message }}
            </p>
            <p
              v-if="r.rejection_reason"
              class="rounded bg-amber-50 border border-amber-200 px-2 py-1 text-xs text-amber-900"
            >
              拒絕理由：{{ r.rejection_reason }}
            </p>
            <button
              v-if="r.status === 'pending'"
              type="button"
              class="text-xs text-slate-500 hover:text-slate-900 underline"
              @click="onCancel(r.id)"
            >
              取消申請
            </button>
          </li>
        </ul>
      </section>

      <div class="grid gap-6 md:grid-cols-2">
        <section class="rounded-xl border border-slate-200 bg-white p-6 shadow-sm">
          <h3 class="text-base font-semibold text-slate-900 mb-4">
            建立新組織
          </h3>
          <OrgCreateForm @created="afterAction" />
        </section>

        <section class="rounded-xl border border-slate-200 bg-white p-6 shadow-sm">
          <h3 class="text-base font-semibold text-slate-900 mb-4">
            加入既有組織
          </h3>
          <OrgJoinForm @joined="afterAction" />
        </section>
      </div>
    </div>
  </main>
</template>
