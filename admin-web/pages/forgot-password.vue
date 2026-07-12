<script setup lang="ts">
definePageMeta({ middleware: 'guest', layout: false })

const auth = useAuth()

const email = ref('')
const submitting = ref(false)
// The endpoint never reveals whether the email matched an account, so the
// UI has exactly one outcome to show on success — no branching on result.
const submitted = ref(false)

async function onSubmit() {
  submitting.value = true
  try {
    await auth.forgotPassword(email.value)
    submitted.value = true
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
        忘記密碼
      </h1>

      <template v-if="!submitted">
        <p class="text-sm text-slate-500 mb-6">
          輸入帳號的 email，我們會寄一封重設密碼的信給你。
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

          <button
            type="submit"
            :disabled="submitting"
            class="w-full rounded-md bg-slate-900 px-3 py-2 text-sm font-medium text-white hover:bg-slate-800 disabled:opacity-60"
          >
            {{ submitting ? '送出中…' : '送出' }}
          </button>
        </form>
      </template>

      <template v-else>
        <p class="text-sm text-slate-600">
          如果 <strong>{{ email }}</strong> 是註冊過的帳號，我們已經寄出一封重設密碼的信，請至信箱查收（連結 60 分鐘內有效）。
        </p>
      </template>

      <p class="mt-6 text-sm text-slate-500 text-center">
        <NuxtLink
          to="/login"
          class="text-slate-900 hover:underline"
        >
          返回登入
        </NuxtLink>
      </p>
    </div>
  </main>
</template>
