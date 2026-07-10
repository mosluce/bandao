<script setup lang="ts">
import type { EncryptMode, ExternalAuthInput, OrgAuthSource, TestLoginResponse } from '~/types/api'
import { ApiError } from '~/types/api'

definePageMeta({ middleware: 'auth' })

const auth = useAuth()
const externalAuth = useExternalAuth()

const currentSource = computed<OrgAuthSource>(
  () => auth.currentOrg.value?.auth_source ?? 'internal',
)

// --- form state ---
const source = ref<OrgAuthSource>('internal')
const host = ref('')
const port = ref(1433)
const database = ref('')
const username = ref('')
const password = ref('') // write-only; blank = keep stored
const query = ref('SELECT emp_id, name FROM staff WHERE acct = @account AND pwd = @password')
const keyCol = ref('emp_id')
const displayCol = ref('name')
const passwordSet = ref(false)
const encrypt = ref<EncryptMode>('optional')
const trustServerCertificate = ref(true)

const saving = ref(false)
const saveError = ref('')
const saved = ref(false)

// --- test-login state ---
const testAccount = ref('')
const testPassword = ref('')
const testing = ref(false)
const testResult = ref<TestLoginResponse | null>(null)
const testError = ref('')

// --- mode-switch confirmation ---
const showSwitchConfirm = ref(false)

/** Load the form from the current org's stored config (password never arrives;
 * we only learn whether one is set). */
function hydrate() {
  const org = auth.currentOrg.value
  source.value = org?.auth_source ?? 'internal'
  const ext = org?.external_auth
  if (ext) {
    host.value = ext.host
    port.value = ext.port
    database.value = ext.database
    username.value = ext.username
    query.value = ext.query
    keyCol.value = ext.key_col
    displayCol.value = ext.display_col
    passwordSet.value = ext.password_set
    encrypt.value = ext.encrypt
    trustServerCertificate.value = ext.trust_server_certificate
  }
  password.value = ''
}

watch(() => auth.currentOrg.value?.id, () => hydrate(), { immediate: true })

function buildInput(): ExternalAuthInput {
  return {
    driver: 'mssql',
    host: host.value.trim(),
    port: Number(port.value),
    database: database.value.trim(),
    username: username.value.trim(),
    // Omit when blank so the stored password is preserved.
    ...(password.value ? { password: password.value } : {}),
    query: query.value.trim(),
    key_col: keyCol.value.trim(),
    display_col: displayCol.value.trim(),
    encrypt: encrypt.value,
    trust_server_certificate: trustServerCertificate.value,
  }
}

function onSaveClick() {
  saveError.value = ''
  saved.value = false
  // Switching auth source locks out the current set of users — confirm first.
  if (source.value !== currentSource.value) {
    showSwitchConfirm.value = true
    return
  }
  void save()
}

async function save() {
  showSwitchConfirm.value = false
  saving.value = true
  saveError.value = ''
  try {
    await externalAuth.configure({
      auth_source: source.value,
      external_auth: source.value === 'external_db' ? buildInput() : undefined,
    })
    await auth.refresh()
    hydrate()
    saved.value = true
  }
  catch (err) {
    if (err instanceof ApiError) {
      saveError.value = err.code === 'FORBIDDEN'
        ? '只有管理員可以變更驗證設定'
        : err.code === 'VALIDATION'
          ? `設定無效：${err.message}`
          : err.message
    }
    else {
      saveError.value = err instanceof Error ? err.message : '儲存失敗'
    }
  }
  finally {
    saving.value = false
  }
}

async function runTestLogin() {
  testError.value = ''
  testResult.value = null
  testing.value = true
  try {
    testResult.value = await externalAuth.testLogin({
      external_auth: buildInput(),
      test_account: testAccount.value,
      test_password: testPassword.value,
    })
  }
  catch (err) {
    testError.value = err instanceof Error ? err.message : '測試失敗'
  }
  finally {
    testing.value = false
  }
}

const switchWarning = computed(() => {
  if (source.value === 'external_db') {
    return '切換到「外部資料庫」後，目前以內建帳號登入的 App 使用者將無法登入（資料與打卡歷史保留，切回即可恢復）。'
  }
  return '切回「內建」後，曾以外部帳號登入的使用者將無法登入（他們的密碼在外部系統）。內建帳號會恢復可登入。'
})
</script>

<template>
  <div class="mx-auto max-w-3xl space-y-6 p-6">
    <div>
      <NuxtLink to="/" class="text-sm text-slate-500 hover:text-slate-900">
        ← 回儀表板
      </NuxtLink>
      <h1 class="mt-2 text-2xl font-semibold text-slate-900">
        驗證來源
      </h1>
      <p class="text-sm text-slate-500">
        設定 App 使用者登入時，帳號密碼要用內建資料庫還是你的外部系統（MSSQL）驗證。
      </p>
    </div>

    <div
      v-if="!auth.isAdmin.value"
      class="rounded-xl border border-amber-200 bg-amber-50 p-6 text-sm text-amber-800"
    >
      只有管理員可以檢視或變更驗證設定。
    </div>

    <template v-else>
      <section class="rounded-xl border border-slate-200 bg-white p-6 shadow-sm space-y-5">
        <div class="space-y-2">
          <label class="flex items-start gap-3">
            <input v-model="source" type="radio" value="internal" class="mt-1">
            <span>
              <span class="font-medium text-slate-900">內建</span>
              <span class="block text-xs text-slate-500">系統管理帳號 + 一次性初始密碼。</span>
            </span>
          </label>
          <label class="flex items-start gap-3">
            <input v-model="source" type="radio" value="external_db" class="mt-1">
            <span>
              <span class="font-medium text-slate-900">外部資料庫</span>
              <span class="block text-xs text-slate-500">用你自己的 MSSQL 帳號系統驗證。</span>
            </span>
          </label>
        </div>

        <div v-if="source === 'external_db'" class="space-y-4 border-t border-slate-100 pt-4">
          <div class="grid grid-cols-1 gap-4 sm:grid-cols-2">
            <label class="text-sm">
              <span class="mb-1 block text-slate-600">資料庫類型</span>
              <input value="MSSQL" disabled class="w-full rounded border border-slate-200 bg-slate-50 px-2 py-1.5 text-slate-500">
            </label>
            <label class="text-sm">
              <span class="mb-1 block text-slate-600">連接埠</span>
              <input v-model.number="port" type="number" class="w-full rounded border border-slate-300 px-2 py-1.5">
            </label>
            <label class="text-sm sm:col-span-2">
              <span class="mb-1 block text-slate-600">主機</span>
              <input v-model="host" type="text" placeholder="10.0.1.20" class="w-full rounded border border-slate-300 px-2 py-1.5 font-mono">
            </label>
            <label class="text-sm">
              <span class="mb-1 block text-slate-600">資料庫</span>
              <input v-model="database" type="text" class="w-full rounded border border-slate-300 px-2 py-1.5 font-mono">
            </label>
            <label class="text-sm">
              <span class="mb-1 block text-slate-600">連線帳號</span>
              <input v-model="username" type="text" class="w-full rounded border border-slate-300 px-2 py-1.5 font-mono">
            </label>
            <label class="text-sm sm:col-span-2">
              <span class="mb-1 block text-slate-600">
                連線密碼
                <span v-if="passwordSet" class="text-xs text-slate-400">（已設定；留空表示不變更）</span>
              </span>
              <input
                v-model="password"
                type="password"
                autocomplete="new-password"
                :placeholder="passwordSet ? '●●●●●●●●' : ''"
                class="w-full rounded border border-slate-300 px-2 py-1.5 font-mono"
              >
            </label>
            <label class="text-sm">
              <span class="mb-1 block text-slate-600">加密 (Encrypt)</span>
              <select v-model="encrypt" class="w-full rounded border border-slate-300 px-2 py-1.5">
                <option value="off">關閉 (off)</option>
                <option value="optional">選用 (optional)</option>
                <option value="required">強制 (required)</option>
              </select>
            </label>
            <label class="flex items-center gap-2 text-sm sm:col-span-1">
              <input v-model="trustServerCertificate" type="checkbox" class="rounded border-slate-300">
              <span class="text-slate-600">信任伺服器憑證</span>
            </label>
            <p class="text-xs text-slate-400 sm:col-span-2">
              舊版地端 MSSQL 常不支援 TLS，連不上時可將加密設為「選用」或「關閉」。憑證為自簽時勾選「信任伺服器憑證」。
            </p>
          </div>

          <label class="block text-sm">
            <span class="mb-1 block text-slate-600">驗證查詢（用 <code>@account</code> / <code>@password</code> 當佔位符）</span>
            <textarea v-model="query" rows="3" class="w-full rounded border border-slate-300 px-2 py-1.5 font-mono text-xs" />
          </label>

          <div class="grid grid-cols-1 gap-4 sm:grid-cols-2">
            <label class="text-sm">
              <span class="mb-1 block text-slate-600">唯一識別欄</span>
              <input v-model="keyCol" type="text" class="w-full rounded border border-slate-300 px-2 py-1.5 font-mono">
            </label>
            <label class="text-sm">
              <span class="mb-1 block text-slate-600">顯示名稱欄</span>
              <input v-model="displayCol" type="text" class="w-full rounded border border-slate-300 px-2 py-1.5 font-mono">
            </label>
          </div>
        </div>

        <div v-if="saveError" class="text-sm text-red-600">
          {{ saveError }}
        </div>
        <div v-if="saved" class="text-sm text-green-700">
          已儲存。
        </div>

        <div class="flex justify-end">
          <button
            type="button"
            :disabled="saving"
            class="rounded-md bg-slate-900 px-4 py-2 text-sm font-medium text-white hover:bg-slate-800 disabled:opacity-60"
            @click="onSaveClick"
          >
            {{ saving ? '儲存中…' : '儲存驗證設定' }}
          </button>
        </div>
      </section>

      <!-- Test-login (dry-run) -->
      <section
        v-if="source === 'external_db'"
        class="rounded-xl border border-slate-200 bg-white p-6 shadow-sm space-y-4"
      >
        <div>
          <h2 class="text-lg font-semibold text-slate-900">
            試登入
          </h2>
          <p class="text-sm text-slate-500">
            用一組真實帳密實際連線測試，不會建立 session 或使用者。
          </p>
        </div>
        <div class="flex flex-wrap items-end gap-3">
          <label class="text-sm">
            <span class="mb-1 block text-slate-600">測試帳號</span>
            <input v-model="testAccount" type="text" class="rounded border border-slate-300 px-2 py-1.5 font-mono">
          </label>
          <label class="text-sm">
            <span class="mb-1 block text-slate-600">測試密碼</span>
            <input v-model="testPassword" type="password" autocomplete="off" class="rounded border border-slate-300 px-2 py-1.5 font-mono">
          </label>
          <button
            type="button"
            :disabled="testing"
            class="rounded-md border border-slate-300 bg-white px-3 py-2 text-sm font-medium text-slate-700 hover:bg-slate-50 disabled:opacity-60"
            @click="runTestLogin"
          >
            {{ testing ? '測試中…' : '試登入' }}
          </button>
        </div>

        <div v-if="testError" class="text-sm text-red-600">
          {{ testError }}
        </div>
        <div
          v-else-if="testResult"
          class="rounded-md border p-3 text-sm"
          :class="testResult.connected
            ? (testResult.matched ? 'border-green-200 bg-green-50' : 'border-amber-200 bg-amber-50')
            : 'border-red-200 bg-red-50'"
        >
          <template v-if="!testResult.connected">
            <p class="font-medium text-red-700">
              連線 / 設定有問題
            </p>
            <p class="text-red-600">
              {{ testResult.error }}
            </p>
          </template>
          <template v-else-if="testResult.matched">
            <p class="font-medium text-green-700">
              ✓ 連線成功，查詢回傳 1 筆
            </p>
            <p class="text-slate-700">
              唯一識別 <code>{{ keyCol }}</code> = <strong>{{ testResult.external_key }}</strong>
            </p>
            <p class="text-slate-700">
              顯示名稱 <code>{{ displayCol }}</code> = <strong>{{ testResult.display_name }}</strong>
            </p>
          </template>
          <template v-else>
            <p class="font-medium text-amber-800">
              連線成功，但這組帳密查不到資料
            </p>
            <p class="text-amber-700">
              確認測試帳密正確，或檢查查詢的帳號 / 密碼欄位。
            </p>
          </template>
        </div>
      </section>
    </template>

    <!-- Mode-switch confirmation -->
    <div
      v-if="showSwitchConfirm"
      class="fixed inset-0 z-50 flex items-center justify-center bg-black/30 p-4"
    >
      <div class="w-full max-w-md rounded-xl bg-white p-6 shadow-xl space-y-4">
        <h3 class="text-lg font-semibold text-slate-900">
          確認切換驗證方式
        </h3>
        <p class="text-sm text-slate-600">
          {{ switchWarning }}
        </p>
        <div class="flex justify-end gap-2">
          <button
            type="button"
            class="rounded-md border border-slate-300 px-3 py-2 text-sm font-medium text-slate-700 hover:bg-slate-50"
            @click="showSwitchConfirm = false"
          >
            取消
          </button>
          <button
            type="button"
            :disabled="saving"
            class="rounded-md bg-slate-900 px-3 py-2 text-sm font-medium text-white hover:bg-slate-800 disabled:opacity-60"
            @click="save"
          >
            確定切換
          </button>
        </div>
      </div>
    </div>
  </div>
</template>
