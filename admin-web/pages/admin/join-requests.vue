<script setup lang="ts">
import type { OrgPendingJoinRequestDto, JoinRequestStatus } from '~/types/api'
import { ApiError } from '~/types/api'

definePageMeta({ middleware: 'auth' })

const auth = useAuth()
const joinRequests = useJoinRequests()
const router = useRouter()

const filterStatus = ref<JoinRequestStatus>('pending')
const requests = ref<OrgPendingJoinRequestDto[]>([])
const loading = ref(false)
const loadError = ref('')

const rejectingId = ref<string | null>(null)
const rejectReason = ref('')
const actionError = ref('')

if (!auth.isAdmin.value) {
  // Member without admin role — bounce to home.
  router.replace('/')
}

async function load() {
  loading.value = true
  loadError.value = ''
  try {
    requests.value = await joinRequests.listOrgPending(filterStatus.value)
  }
  catch (err) {
    loadError.value = err instanceof Error ? err.message : '載入失敗'
  }
  finally {
    loading.value = false
  }
}

watch(
  [filterStatus, () => auth.currentOrg.value?.id],
  () => {
    if (auth.isAdmin.value && auth.currentOrg.value) load()
  },
  { immediate: true },
)

async function onApprove(id: string) {
  actionError.value = ''
  try {
    await joinRequests.approve(id)
    await load()
  }
  catch (err) {
    if (err instanceof ApiError && err.code === 'EMAIL_IN_COOLDOWN') {
      actionError.value = '此 email 在冷卻期內，無法批准。請等冷卻結束或先清除冷卻紀錄。'
    }
    else {
      actionError.value = err instanceof Error ? err.message : '批准失敗'
    }
  }
}

function openRejectModal(id: string) {
  rejectingId.value = id
  rejectReason.value = ''
  actionError.value = ''
}

function closeRejectModal() {
  rejectingId.value = null
  rejectReason.value = ''
}

async function confirmReject() {
  if (!rejectingId.value) return
  if (rejectReason.value.length > 500) {
    actionError.value = '拒絕理由最多 500 字'
    return
  }
  try {
    await joinRequests.reject(rejectingId.value, rejectReason.value || undefined)
    closeRejectModal()
    await load()
  }
  catch (err) {
    actionError.value = err instanceof Error ? err.message : '拒絕失敗'
  }
}

const statusLabel: Record<JoinRequestStatus, string> = {
  pending: '待審核',
  approved: '已批准',
  rejected: '已拒絕',
  cancelled: '已取消',
}
</script>

<template>
  <main class="min-h-screen px-4 py-10">
    <div class="max-w-4xl mx-auto space-y-6">
      <header>
        <h1 class="text-2xl font-semibold text-slate-900">
          加入申請
        </h1>
        <p class="text-sm text-slate-500">
          {{ auth.currentOrg.value?.name }}
        </p>
      </header>

      <div class="flex flex-wrap gap-2">
        <button
          v-for="s in (['pending', 'approved', 'rejected', 'cancelled'] as JoinRequestStatus[])"
          :key="s"
          type="button"
          class="rounded-full border px-3 py-1 text-xs font-medium"
          :class="filterStatus === s
            ? 'border-slate-900 bg-slate-900 text-white'
            : 'border-slate-300 bg-white text-slate-700 hover:bg-slate-50'"
          @click="filterStatus = s"
        >
          {{ statusLabel[s] }}
        </button>
      </div>

      <p v-if="loadError" class="text-sm text-red-600">
        {{ loadError }}
      </p>
      <p v-if="actionError" class="text-sm text-red-600">
        {{ actionError }}
      </p>

      <div v-if="loading" class="text-sm text-slate-500">
        載入中...
      </div>
      <div v-else-if="requests.length === 0" class="rounded-xl border border-slate-200 bg-white p-12 text-center text-sm text-slate-500">
        目前沒有 {{ statusLabel[filterStatus] }} 的申請
      </div>
      <ul v-else class="space-y-3">
        <li
          v-for="r in requests"
          :key="r.id"
          class="rounded-xl border border-slate-200 bg-white p-4 space-y-2"
        >
          <div class="flex flex-wrap items-baseline justify-between gap-2">
            <div>
              <p class="font-medium text-slate-900">
                {{ r.email }}
              </p>
              <p class="text-xs text-slate-500">
                {{ r.requested_at }}
              </p>
            </div>
            <div v-if="r.status === 'pending'" class="flex gap-2">
              <button
                type="button"
                class="rounded-md bg-slate-900 px-3 py-1.5 text-sm font-medium text-white hover:bg-slate-700"
                @click="onApprove(r.id)"
              >
                同意
              </button>
              <button
                type="button"
                class="rounded-md border border-slate-300 bg-white px-3 py-1.5 text-sm font-medium text-slate-700 hover:bg-slate-50"
                @click="openRejectModal(r.id)"
              >
                拒絕
              </button>
            </div>
            <span
              v-else
              class="text-xs text-slate-500"
            >
              {{ statusLabel[r.status] }}{{ r.decided_at ? ` · ${r.decided_at}` : '' }}
            </span>
          </div>
          <p
            v-if="r.application_message"
            class="rounded-md bg-slate-50 p-3 text-sm text-slate-700 whitespace-pre-wrap"
          >
            {{ r.application_message }}
          </p>
          <p
            v-if="r.rejection_reason"
            class="rounded-md bg-amber-50 border border-amber-200 p-3 text-sm text-amber-900"
          >
            拒絕理由：{{ r.rejection_reason }}
          </p>
        </li>
      </ul>
    </div>

    <Teleport to="body">
      <div
        v-if="rejectingId"
        class="fixed inset-0 z-[1100] flex items-center justify-center bg-slate-900/40 px-4"
      >
        <div class="w-full max-w-md rounded-xl bg-white p-6 shadow-lg space-y-4">
          <h2 class="text-lg font-semibold text-slate-900">
            拒絕加入申請
          </h2>
          <label class="block text-sm">
            <span class="text-slate-700">拒絕理由（可選，最多 500 字）</span>
            <textarea
              v-model="rejectReason"
              rows="4"
              maxlength="500"
              class="mt-1 w-full rounded-md border border-slate-300 px-3 py-2"
            />
          </label>
          <p
            v-if="actionError"
            class="text-xs text-red-600"
          >
            {{ actionError }}
          </p>
          <div class="flex justify-end gap-2">
            <button
              type="button"
              class="rounded-md border border-slate-300 bg-white px-3 py-2 text-sm font-medium text-slate-700 hover:bg-slate-50"
              @click="closeRejectModal"
            >
              取消
            </button>
            <button
              type="button"
              class="rounded-md bg-slate-900 px-3 py-2 text-sm font-medium text-white hover:bg-slate-700"
              @click="confirmReject"
            >
              確認拒絕
            </button>
          </div>
        </div>
      </div>
    </Teleport>
  </main>
</template>
