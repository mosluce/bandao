<script setup lang="ts">
interface NavItem {
  to: string
  label: string
  badge?: number
}

const auth = useAuth()
const joinRequests = useJoinRequests()
const router = useRouter()

const sidebarOpen = ref(false)

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

const navItems = computed<NavItem[]>(() => {
  const base: NavItem[] = [
    { to: '/members', label: '成員管理' },
    { to: '/app-users', label: 'App 使用者' },
    { to: '/checkin', label: '打卡看板' },
  ]
  if (auth.isAdmin.value) {
    base.push(
      { to: '/cooldowns', label: '冷卻管理' },
      {
        to: '/admin/join-requests',
        label: '加入申請',
        badge: pendingJoinCount.value > 0 ? pendingJoinCount.value : undefined,
      },
      { to: '/settings/auth', label: '驗證來源' },
      { to: '/settings/api-tokens', label: 'API Token' },
    )
  }
  base.push({ to: '/download', label: '下載 App' })
  return base
})

function closeSidebarOnNavigate() {
  sidebarOpen.value = false
}

async function onLogout() {
  await auth.logout()
  await navigateTo('/login')
}

function goHome() {
  router.push('/')
}
</script>

<template>
  <div class="flex min-h-screen bg-slate-50">
    <!-- Narrow-viewport backdrop, closes the panel on tap outside it. -->
    <div
      v-if="sidebarOpen"
      class="fixed inset-0 z-30 bg-slate-900/40 md:hidden"
      @click="sidebarOpen = false"
    />

    <aside
      class="fixed inset-y-0 left-0 z-40 flex w-64 shrink-0 transform flex-col border-r border-slate-200 bg-white transition-transform duration-200 md:static md:translate-x-0"
      :class="sidebarOpen ? 'translate-x-0' : '-translate-x-full'"
    >
      <div class="border-b border-slate-200 p-4">
        <button
          type="button"
          class="text-left text-lg font-semibold text-slate-900 hover:text-slate-700"
          @click="goHome(); closeSidebarOnNavigate()"
        >
          班到 admin
        </button>
        <OrgSwitcher class="mt-2" />
      </div>

      <nav class="flex-1 space-y-1 overflow-y-auto p-4">
        <NuxtLink
          v-for="item in navItems"
          :key="item.to"
          :to="item.to"
          class="relative flex items-center rounded-md px-3 py-2 text-sm font-medium text-slate-700 hover:bg-slate-100"
          active-class="bg-slate-100 text-slate-900"
          @click="closeSidebarOnNavigate"
        >
          {{ item.label }}
          <span
            v-if="item.badge"
            class="ml-auto inline-flex min-w-5 items-center justify-center rounded-full bg-red-500 px-1.5 text-xs font-semibold text-white"
          >
            {{ item.badge }}
          </span>
        </NuxtLink>
      </nav>

      <div class="border-t border-slate-200 p-4">
        <p
          v-if="auth.user.value"
          class="truncate text-xs text-slate-500"
        >
          {{ auth.user.value.email }}
        </p>
        <button
          type="button"
          class="mt-2 w-full rounded-md border border-slate-300 bg-white px-3 py-2 text-sm font-medium text-slate-700 hover:bg-slate-50"
          @click="onLogout"
        >
          登出
        </button>
      </div>
    </aside>

    <div class="min-w-0 flex-1">
      <div class="sticky top-0 z-20 flex items-center gap-3 border-b border-slate-200 bg-white px-4 py-3 md:hidden">
        <button
          type="button"
          class="rounded-md border border-slate-300 p-2 text-slate-700 hover:bg-slate-50"
          aria-label="開啟選單"
          @click="sidebarOpen = true"
        >
          <svg
            class="h-5 w-5"
            fill="none"
            viewBox="0 0 24 24"
            stroke="currentColor"
          >
            <path
              stroke-linecap="round"
              stroke-linejoin="round"
              stroke-width="2"
              d="M4 6h16M4 12h16M4 18h16"
            />
          </svg>
        </button>
        <span class="truncate text-sm font-medium text-slate-700">
          {{ auth.currentOrg.value?.name || '班到 admin' }}
        </span>
      </div>

      <slot />
    </div>
  </div>
</template>
