<script setup lang="ts">
import { ApiError } from '~/types/api'

const emit = defineEmits<{
  joined: []
}>()

const auth = useAuth()
const orgCode = ref('')
const submitting = ref(false)
const errorMessage = ref('')

const CODE_SHAPED = /^[2-9A-HJ-NP-Za-hj-np-z]{10}$/

function normalizeOrgCode(input: string): string {
  const trimmed = input.trim()
  return CODE_SHAPED.test(trimmed) ? trimmed.toUpperCase() : trimmed.toLowerCase()
}

async function onSubmit() {
  errorMessage.value = ''
  submitting.value = true
  try {
    await auth.joinOrg(normalizeOrgCode(orgCode.value))
    orgCode.value = ''
    emit('joined')
  }
  catch (err) {
    if (err instanceof ApiError) {
      errorMessage.value = friendly(err)
    }
    else {
      errorMessage.value = err instanceof Error ? err.message : '加入失敗'
    }
  }
  finally {
    submitting.value = false
  }
}

function friendly(err: ApiError): string {
  switch (err.code) {
    case 'INVALID_ORG_CODE':
      return '組織代碼無效或已失效'
    case 'ALREADY_MEMBER':
      return '你已經是此組織成員'
    case 'EMAIL_IN_COOLDOWN':
      return '此 email 在這個組織的 7 天冷卻期內，無法重新加入'
    default:
      return err.message
  }
}
</script>

<template>
  <form
    class="space-y-4"
    @submit.prevent="onSubmit"
  >
    <div>
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
        :disabled="submitting"
        class="w-full rounded-md border border-slate-300 px-3 py-2 font-mono tracking-wider text-sm focus:border-slate-900 focus:outline-none focus:ring-1 focus:ring-slate-900"
      >
      <p class="text-xs text-slate-500 mt-1">
        10 位隨機代碼或組織自訂代碼，由組織 admin 提供。
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
      {{ submitting ? '加入中…' : '加入組織' }}
    </button>
  </form>
</template>
