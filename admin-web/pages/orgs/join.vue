<script setup lang="ts">
definePageMeta({ middleware: 'auth' })

const auth = useAuth()

async function onJoined() {
  await navigateTo('/')
}

const hasMemberships = computed(() => auth.memberships.value.length > 0)
</script>

<template>
  <main class="min-h-screen px-4 py-10">
    <div class="max-w-md mx-auto space-y-6">
      <header class="space-y-1">
        <NuxtLink
          v-if="hasMemberships"
          to="/"
          class="text-xs text-slate-500 hover:text-slate-900"
        >
          ← 回首頁
        </NuxtLink>
        <NuxtLink
          v-else
          to="/no-org"
          class="text-xs text-slate-500 hover:text-slate-900"
        >
          ← 返回
        </NuxtLink>
        <h1 class="text-2xl font-semibold text-slate-900">
          加入既有組織
        </h1>
      </header>

      <section class="rounded-xl border border-slate-200 bg-white p-6 shadow-sm">
        <OrgJoinForm @joined="onJoined" />
      </section>
    </div>
  </main>
</template>
