<script setup lang="ts">
import { ApiError } from '~/types/api'

definePageMeta({ middleware: 'guest', layout: false })

const auth = useAuth()
const route = useRoute()

const email = ref('')
const password = ref('')
const errorMessage = ref('')
const submitting = ref(false)

async function onSubmit() {
  errorMessage.value = ''
  submitting.value = true
  try {
    await auth.login({ email: email.value, password: password.value })
    const next = typeof route.query.next === 'string' ? route.query.next : '/'
    await navigateTo(next)
  }
  catch (err) {
    if (err instanceof ApiError && err.code === 'INVALID_CREDENTIALS') {
      errorMessage.value = '帳號或密碼錯誤'
    }
    else {
      errorMessage.value = err instanceof Error ? err.message : '登入失敗'
    }
  }
  finally {
    submitting.value = false
  }
}
</script>

<template>
  <main class="min-h-screen flex items-center justify-center px-4">
    <div class="w-full max-w-md bg-white border border-slate-200 rounded-xl shadow-sm p-8">
      <h1 class="text-2xl font-semibold text-slate-900 mb-1">
        登入班到
      </h1>
      <p class="text-sm text-slate-500 mb-6">
        使用 dashboard 帳號登入
      </p>

      <form
        class="space-y-4"
        @submit.prevent="onSubmit"
      >
        <div>
          <label
            for="email"
            class="block text-sm font-medium text-slate-700 mb-1"
          >Email</label>
          <input
            id="email"
            v-model="email"
            type="email"
            autocomplete="email"
            required
            class="w-full rounded-md border border-slate-300 px-3 py-2 text-sm focus:border-slate-900 focus:outline-none focus:ring-1 focus:ring-slate-900"
          >
        </div>

        <div>
          <label
            for="password"
            class="block text-sm font-medium text-slate-700 mb-1"
          >密碼</label>
          <input
            id="password"
            v-model="password"
            type="password"
            autocomplete="current-password"
            required
            minlength="8"
            class="w-full rounded-md border border-slate-300 px-3 py-2 text-sm focus:border-slate-900 focus:outline-none focus:ring-1 focus:ring-slate-900"
          >
        </div>

        <p
          v-if="errorMessage"
          class="text-sm text-red-600"
        >
          {{ errorMessage }}
        </p>

        <button
          type="submit"
          :disabled="submitting"
          class="w-full rounded-md bg-slate-900 px-3 py-2 text-sm font-medium text-white hover:bg-slate-800 disabled:opacity-60"
        >
          {{ submitting ? '登入中…' : '登入' }}
        </button>
      </form>

      <p class="mt-6 text-sm text-slate-500 text-center">
        還沒有帳號？
        <NuxtLink
          to="/register"
          class="text-slate-900 hover:underline"
        >
          建立或加入組織
        </NuxtLink>
      </p>
    </div>
  </main>
</template>
