<script setup lang="ts">
import { ApiError } from '~/types/api'

const emit = defineEmits<{
  created: []
}>()

const auth = useAuth()
const orgName = ref('')
const submitting = ref(false)
const errorMessage = ref('')

async function onSubmit() {
  errorMessage.value = ''
  submitting.value = true
  try {
    await auth.createOrg(orgName.value.trim())
    orgName.value = ''
    emit('created')
  }
  catch (err) {
    if (err instanceof ApiError) {
      errorMessage.value = err.code === 'VALIDATION'
        ? err.message
        : err.message || '建立失敗'
    }
    else {
      errorMessage.value = err instanceof Error ? err.message : '建立失敗'
    }
  }
  finally {
    submitting.value = false
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
        for="orgName"
        class="block text-sm font-medium text-slate-700 mb-1"
      >組織名稱</label>
      <input
        id="orgName"
        v-model="orgName"
        type="text"
        required
        maxlength="120"
        :disabled="submitting"
        class="w-full rounded-md border border-slate-300 px-3 py-2 text-sm focus:border-slate-900 focus:outline-none focus:ring-1 focus:ring-slate-900"
      >
      <p class="text-xs text-slate-500 mt-1">
        建立後你將成為該組織的擁有者。
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
      {{ submitting ? '建立中…' : '建立組織' }}
    </button>
  </form>
</template>
