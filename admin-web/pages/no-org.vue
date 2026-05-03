<script setup lang="ts">
definePageMeta({ middleware: 'auth' })

const auth = useAuth()

async function onLogout() {
  await auth.logout()
  await navigateTo('/login')
}

async function afterAction() {
  await navigateTo('/')
}
</script>

<template>
  <main class="min-h-screen px-4 py-10">
    <div class="max-w-2xl mx-auto space-y-6">
      <header class="flex items-center justify-between">
        <div>
          <h1 class="text-2xl font-semibold text-slate-900">
            argus admin
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
          建立一個新組織當擁有者，或用組織代碼加入既有組織。
        </p>
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
