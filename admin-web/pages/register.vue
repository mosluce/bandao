<script setup lang="ts">
import { ApiError } from '~/types/api'

definePageMeta({ middleware: 'guest', layout: false })

const auth = useAuth()
const route = useRoute()

type Mode = 'create' | 'join'
const mode = ref<Mode>(typeof route.query.code === 'string' && route.query.code ? 'join' : 'create')

const email = ref('')
const password = ref('')
const orgName = ref('')
const orgCode = ref(typeof route.query.code === 'string' ? route.query.code.trim() : '')

const errorMessage = ref('')
const submitting = ref(false)

const CODE_SHAPED = /^[2-9A-HJ-NP-Za-hj-np-z]{10}$/

function normalizeOrgCode(input: string): string {
  const trimmed = input.trim()
  return CODE_SHAPED.test(trimmed) ? trimmed.toUpperCase() : trimmed.toLowerCase()
}

async function onSubmit() {
  errorMessage.value = ''
  submitting.value = true
  try {
    if (mode.value === 'create') {
      await auth.register({
        mode: 'create',
        email: email.value,
        password: password.value,
        org_name: orgName.value,
      })
    }
    else {
      await auth.register({
        mode: 'join',
        email: email.value,
        password: password.value,
        org_code: normalizeOrgCode(orgCode.value),
      })
    }
    await navigateTo('/')
  }
  catch (err) {
    if (err instanceof ApiError) {
      errorMessage.value = friendlyError(err)
    }
    else {
      errorMessage.value = err instanceof Error ? err.message : '註冊失敗'
    }
  }
  finally {
    submitting.value = false
  }
}

function friendlyError(err: ApiError): string {
  switch (err.code) {
    case 'EMAIL_TAKEN':
      return '此 email 已被使用'
    case 'INVALID_ORG_CODE':
      return '組織代碼無效或已失效'
    case 'VALIDATION':
      return err.message
    default:
      return err.message
  }
}
</script>

<template>
  <main class="min-h-screen flex items-center justify-center px-4">
    <div class="w-full max-w-md bg-white border border-slate-200 rounded-xl shadow-sm p-8">
      <h1 class="text-2xl font-semibold text-slate-900 mb-1">
        建立或加入組織
      </h1>
      <p class="text-sm text-slate-500 mb-6">
        建立新組織會讓你成為該組織的第一位 admin
      </p>

      <div class="flex rounded-md border border-slate-200 p-1 mb-6 text-sm">
        <button
          type="button"
          class="flex-1 rounded px-3 py-1.5 transition"
          :class="mode === 'create' ? 'bg-slate-900 text-white' : 'text-slate-600 hover:text-slate-900'"
          @click="mode = 'create'"
        >
          建立新組織
        </button>
        <button
          type="button"
          class="flex-1 rounded px-3 py-1.5 transition"
          :class="mode === 'join' ? 'bg-slate-900 text-white' : 'text-slate-600 hover:text-slate-900'"
          @click="mode = 'join'"
        >
          加入既有組織
        </button>
      </div>

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
            autocomplete="new-password"
            required
            minlength="8"
            class="w-full rounded-md border border-slate-300 px-3 py-2 text-sm focus:border-slate-900 focus:outline-none focus:ring-1 focus:ring-slate-900"
          >
          <p class="text-xs text-slate-500 mt-1">
            至少 8 個字元
          </p>
        </div>

        <div v-if="mode === 'create'">
          <label
            for="orgName"
            class="block text-sm font-medium text-slate-700 mb-1"
          >組織名稱</label>
          <input
            id="orgName"
            v-model="orgName"
            type="text"
            required
            maxlength="120"
            class="w-full rounded-md border border-slate-300 px-3 py-2 text-sm focus:border-slate-900 focus:outline-none focus:ring-1 focus:ring-slate-900"
          >
        </div>

        <div v-else>
          <label
            for="orgCode"
            class="block text-sm font-medium text-slate-700 mb-1"
          >組織代碼</label>
          <input
            id="orgCode"
            v-model="orgCode"
            type="text"
            required
            minlength="2"
            maxlength="24"
            spellcheck="false"
            autocapitalize="none"
            class="w-full rounded-md border border-slate-300 px-3 py-2 font-mono tracking-wider text-sm focus:border-slate-900 focus:outline-none focus:ring-1 focus:ring-slate-900"
          >
          <p class="text-xs text-slate-500 mt-1">
            10 位隨機代碼或組織自訂代碼，由組織 admin 提供
          </p>
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
          {{ submitting ? '送出中…' : (mode === 'create' ? '建立組織' : '加入組織') }}
        </button>
      </form>

      <p class="mt-6 text-sm text-slate-500 text-center">
        已經有帳號？
        <NuxtLink
          to="/login"
          class="text-slate-900 hover:underline"
        >
          直接登入
        </NuxtLink>
      </p>
    </div>
  </main>
</template>
