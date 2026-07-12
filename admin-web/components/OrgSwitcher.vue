<script setup lang="ts">
import type { MembershipDto } from '~/types/api'

const auth = useAuth()
const router = useRouter()

const open = ref(false)
const switching = ref<string | null>(null)
const switchError = ref('')
const root = ref<HTMLElement | null>(null)

const ownedMemberships = computed(() =>
  auth.memberships.value.filter(m => isOwnedByMe(m)),
)
const joinedMemberships = computed(() =>
  auth.memberships.value.filter(m => !isOwnedByMe(m)),
)

function isOwnedByMe(m: MembershipDto): boolean {
  const uid = auth.user.value?.id
  return !!uid && m.org.owner_id === uid
}

function roleLabel(m: MembershipDto): string {
  if (isOwnedByMe(m)) return '擁有者'
  return m.role === 'admin' ? '管理員' : '成員'
}

function badgeClass(m: MembershipDto): string {
  if (isOwnedByMe(m)) return 'bg-amber-100 text-amber-800'
  return m.role === 'admin' ? 'bg-slate-200 text-slate-700' : 'bg-slate-100 text-slate-600'
}

const currentRoleLabel = computed(() => {
  const o = auth.currentOrg.value
  if (!o) return ''
  if (auth.isOwner.value) return '擁有者'
  return auth.role.value === 'admin' ? '管理員' : '成員'
})

async function pick(m: MembershipDto) {
  if (m.org.id === auth.currentOrg.value?.id) {
    open.value = false
    return
  }
  switchError.value = ''
  switching.value = m.org.id
  try {
    await auth.switchOrg(m.org.id)
    open.value = false
  }
  catch (err) {
    switchError.value = err instanceof Error ? err.message : '切換失敗'
  }
  finally {
    switching.value = null
  }
}

function go(path: string) {
  open.value = false
  router.push(path)
}

function onDocumentClick(ev: MouseEvent) {
  if (!root.value) return
  if (!root.value.contains(ev.target as Node)) open.value = false
}

function onKeydown(ev: KeyboardEvent) {
  if (ev.key === 'Escape') open.value = false
}

onMounted(() => {
  document.addEventListener('click', onDocumentClick)
  document.addEventListener('keydown', onKeydown)
})
onBeforeUnmount(() => {
  document.removeEventListener('click', onDocumentClick)
  document.removeEventListener('keydown', onKeydown)
})
</script>

<template>
  <div
    v-if="auth.currentOrg.value"
    ref="root"
    class="relative block w-full text-left"
  >
    <button
      type="button"
      class="inline-flex items-center gap-2 rounded-md border border-slate-300 bg-white px-3 py-2 text-sm font-medium text-slate-700 hover:bg-slate-50"
      @click="open = !open"
    >
      <span class="truncate max-w-[12rem]">{{ auth.currentOrg.value.name }}</span>
      <span
        class="rounded px-1.5 py-0.5 text-xs font-normal"
        :class="auth.isOwner.value ? 'bg-amber-100 text-amber-800' : auth.isAdmin.value ? 'bg-slate-200 text-slate-700' : 'bg-slate-100 text-slate-600'"
      >
        {{ currentRoleLabel }}
      </span>
      <svg
        class="h-4 w-4 text-slate-400"
        fill="none"
        viewBox="0 0 20 20"
      >
        <path
          stroke="currentColor"
          stroke-linecap="round"
          stroke-linejoin="round"
          stroke-width="1.5"
          d="m6 8 4 4 4-4"
        />
      </svg>
    </button>

    <div
      v-if="open"
      class="absolute left-0 right-0 z-10 mt-2 origin-top rounded-md border border-slate-200 bg-white shadow-lg"
    >
      <div
        v-if="ownedMemberships.length"
        class="border-b border-slate-100"
      >
        <p class="px-4 pt-3 pb-1 text-xs font-medium uppercase tracking-wide text-slate-400">
          我擁有的
        </p>
        <ul>
          <li
            v-for="m in ownedMemberships"
            :key="m.org.id"
          >
            <button
              type="button"
              :disabled="switching !== null"
              class="flex w-full items-center justify-between gap-2 px-4 py-2 text-left text-sm text-slate-800 hover:bg-slate-50 disabled:opacity-60"
              :class="m.org.id === auth.currentOrg.value?.id ? 'bg-slate-50 font-medium' : ''"
              @click="pick(m)"
            >
              <span class="truncate">{{ m.org.name }}</span>
              <span
                class="shrink-0 rounded px-1.5 py-0.5 text-xs font-normal"
                :class="badgeClass(m)"
              >{{ roleLabel(m) }}</span>
            </button>
          </li>
        </ul>
      </div>

      <div v-if="joinedMemberships.length">
        <p class="px-4 pt-3 pb-1 text-xs font-medium uppercase tracking-wide text-slate-400">
          我加入的
        </p>
        <ul>
          <li
            v-for="m in joinedMemberships"
            :key="m.org.id"
          >
            <button
              type="button"
              :disabled="switching !== null"
              class="flex w-full items-center justify-between gap-2 px-4 py-2 text-left text-sm text-slate-800 hover:bg-slate-50 disabled:opacity-60"
              :class="m.org.id === auth.currentOrg.value?.id ? 'bg-slate-50 font-medium' : ''"
              @click="pick(m)"
            >
              <span class="truncate">{{ m.org.name }}</span>
              <span
                class="shrink-0 rounded px-1.5 py-0.5 text-xs font-normal"
                :class="badgeClass(m)"
              >{{ roleLabel(m) }}</span>
            </button>
          </li>
        </ul>
      </div>

      <div class="border-t border-slate-100 py-1">
        <button
          type="button"
          class="w-full px-4 py-2 text-left text-sm text-slate-700 hover:bg-slate-50"
          @click="go('/orgs/new')"
        >
          + 建立新組織
        </button>
        <button
          type="button"
          class="w-full px-4 py-2 text-left text-sm text-slate-700 hover:bg-slate-50"
          @click="go('/orgs/join')"
        >
          + 用 org code 加入
        </button>
      </div>

      <p
        v-if="switchError"
        class="border-t border-slate-100 px-4 py-2 text-xs text-red-600"
      >
        {{ switchError }}
      </p>
    </div>
  </div>
</template>
