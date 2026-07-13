<script setup lang="ts">
import { ApiError } from '~/types/api'

definePageMeta({ middleware: 'guest', layout: false })

const auth = useAuth()
const route = useRoute()

const token = computed(() => typeof route.query.token === 'string' ? route.query.token : '')

const newPassword = ref('')
const confirmPassword = ref('')
const submitting = ref(false)
const errorMessage = ref('')
const linkInvalid = ref(false)

async function onSubmit() {
  errorMessage.value = ''
  if (newPassword.value !== confirmPassword.value) {
    errorMessage.value = '兩次輸入的密碼不一致'
    return
  }
  submitting.value = true
  try {
    await auth.resetPassword(token.value, newPassword.value)
    await navigateTo('/login?reset=1')
  }
  catch (err) {
    if (err instanceof ApiError && err.code === 'INVALID_RESET_TOKEN') {
      linkInvalid.value = true
    }
    else if (err instanceof ApiError && err.code === 'VALIDATION') {
      errorMessage.value = err.message
    }
    else {
      errorMessage.value = err instanceof Error ? err.message : '重設失敗'
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
        重設密碼
      </h1>

      <template v-if="!token">
        <p class="text-sm text-red-600 mt-4">
          網址缺少重設連結所需的資訊，請重新申請。
        </p>
      </template>

      <template v-else-if="linkInvalid">
        <p class="text-sm text-red-600 mt-4">
          這個連結已經失效或已被使用過，請重新申請。
        </p>
      </template>

      <template v-else>
        <p class="text-sm text-slate-500 mb-6">
          設定新密碼。
        </p>

        <form
          class="space-y-4"
          @submit.prevent="onSubmit"
        >
          <div>
            <label
              for="newPassword"
              class="block text-sm font-medium text-slate-700 mb-1"
            >新密碼</label>
            <input
              id="newPassword"
              v-model="newPassword"
              type="password"
              autocomplete="new-password"
              required
              minlength="8"
              class="w-full rounded-md border border-slate-300 px-3 py-2 text-sm focus:border-slate-900 focus:outline-none focus:ring-1 focus:ring-slate-900"
            >
          </div>

          <div>
            <label
              for="confirmPassword"
              class="block text-sm font-medium text-slate-700 mb-1"
            >確認新密碼</label>
            <input
              id="confirmPassword"
              v-model="confirmPassword"
              type="password"
              autocomplete="new-password"
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
            {{ submitting ? '重設中…' : '重設密碼' }}
          </button>
        </form>
      </template>

      <p class="mt-6 text-sm text-slate-500 text-center">
        <NuxtLink
          to="/forgot-password"
          class="text-slate-900 hover:underline"
        >
          重新申請重設連結
        </NuxtLink>
      </p>
    </div>
  </main>
</template>
