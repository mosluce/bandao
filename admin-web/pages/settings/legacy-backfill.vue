<script setup lang="ts">
import type { CheckinEventType, LegacyBackfillInput, LegacyBackfillJobDto, LegacyBackfillPreviewResponse } from '~/types/api'
import { ApiError } from '~/types/api'

definePageMeta({ middleware: 'auth' })

const auth = useAuth()
const legacyBackfill = useLegacyBackfill()

const EVENT_TYPE_OPTIONS: { value: CheckinEventType, label: string }[] = [
  { value: 'clock_in', label: '上班 (clock_in)' },
  { value: 'clock_out', label: '下班 (clock_out)' },
  { value: 'transfer_out', label: '轉出 (transfer_out)' },
  { value: 'transfer_in', label: '轉入 (transfer_in)' },
]

// --- form state ---
const connectionString = ref('') // write-only; blank = keep stored
const connectionConfigured = ref(false)
const database = ref('')
const collection = ref('')
const identityField = ref('signer.username')
const timestampField = ref('at')
const latField = ref('geo.lat')
const lngField = ref('geo.lng')
const regionNameField = ref('address')
const manualLabelField = ref('comment')
const actionField = ref('action')
const actionMapRows = ref<{ source: string, eventType: CheckinEventType }[]>([])

const saving = ref(false)
const saveError = ref('')
const saved = ref(false)

// --- preview (config-time dry-run) state ---
const testUsername = ref('')
const previewLimit = ref(5)
const previewing = ref(false)
const previewResult = ref<LegacyBackfillPreviewResponse | null>(null)
const previewError = ref('')

// --- job status list ---
const jobs = ref<LegacyBackfillJobDto[]>([])
const jobsError = ref('')
const loadingJobs = ref(false)

function hydrate() {
  const cfg = auth.currentOrg.value?.legacy_backfill
  if (cfg) {
    database.value = cfg.database
    collection.value = cfg.collection
    identityField.value = cfg.identity_field
    timestampField.value = cfg.timestamp_field
    latField.value = cfg.lat_field
    lngField.value = cfg.lng_field
    regionNameField.value = cfg.region_name_field ?? ''
    manualLabelField.value = cfg.manual_label_field ?? ''
    actionField.value = cfg.action_field
    actionMapRows.value = Object.entries(cfg.action_map).map(([source, eventType]) => ({ source, eventType }))
    connectionConfigured.value = cfg.connection_configured
  }
  connectionString.value = ''
}

async function loadJobs() {
  loadingJobs.value = true
  jobsError.value = ''
  try {
    jobs.value = await legacyBackfill.listJobs()
  }
  catch (err) {
    jobsError.value = err instanceof Error ? err.message : '載入失敗'
  }
  finally {
    loadingJobs.value = false
  }
}

watch(() => auth.currentOrg.value?.id, () => {
  hydrate()
  if (auth.isAdmin.value) {
    void loadJobs()
  }
}, { immediate: true })

function addActionRow() {
  actionMapRows.value.push({ source: '', eventType: 'clock_in' })
}

function removeActionRow(index: number) {
  actionMapRows.value.splice(index, 1)
}

function buildInput(): LegacyBackfillInput {
  const actionMap: Record<string, CheckinEventType> = {}
  for (const row of actionMapRows.value) {
    const key = row.source.trim()
    if (key)
      actionMap[key] = row.eventType
  }
  return {
    // Omit when blank so the stored connection string is preserved.
    ...(connectionString.value ? { connection_string: connectionString.value } : {}),
    database: database.value.trim(),
    collection: collection.value.trim(),
    identity_field: identityField.value.trim(),
    timestamp_field: timestampField.value.trim(),
    lat_field: latField.value.trim(),
    lng_field: lngField.value.trim(),
    ...(regionNameField.value.trim() ? { region_name_field: regionNameField.value.trim() } : {}),
    ...(manualLabelField.value.trim() ? { manual_label_field: manualLabelField.value.trim() } : {}),
    action_field: actionField.value.trim(),
    action_map: actionMap,
  }
}

async function save() {
  saving.value = true
  saveError.value = ''
  saved.value = false
  try {
    await legacyBackfill.configure(buildInput())
    await auth.refresh()
    hydrate()
    saved.value = true
  }
  catch (err) {
    if (err instanceof ApiError) {
      saveError.value = err.code === 'FORBIDDEN'
        ? '只有管理員可以變更此設定'
        : err.code === 'LEGACY_BACKFILL_UNAVAILABLE'
          ? '伺服器尚未設定加密金鑰，請聯絡維運人員'
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

async function runPreview() {
  previewError.value = ''
  previewResult.value = null
  previewing.value = true
  try {
    previewResult.value = await legacyBackfill.preview({
      legacy_backfill: buildInput(),
      test_username: testUsername.value.trim(),
      limit: previewLimit.value,
    })
  }
  catch (err) {
    previewError.value = err instanceof Error ? err.message : '預覽失敗'
  }
  finally {
    previewing.value = false
  }
}

const JOB_STATUS_LABEL: Record<string, string> = {
  pending: '等待中',
  active: '執行中',
  done: '已完成',
  failed: '失敗',
}

function jobStatusClass(status: string): string {
  switch (status) {
    case 'done':
      return 'bg-green-50 text-green-700 border-green-200'
    case 'failed':
      return 'bg-red-50 text-red-700 border-red-200'
    case 'active':
      return 'bg-blue-50 text-blue-700 border-blue-200'
    default:
      return 'bg-slate-50 text-slate-700 border-slate-200'
  }
}
</script>

<template>
  <div class="mx-auto max-w-3xl space-y-6 p-6">
    <div>
      <NuxtLink to="/" class="text-sm text-slate-500 hover:text-slate-900">
        ← 回儀表板
      </NuxtLink>
      <h1 class="mt-2 text-2xl font-semibold text-slate-900">
        舊系統資料回填
      </h1>
      <p class="text-sm text-slate-500">
        設定客戶舊打卡系統（MongoDB）的連線與欄位對應。設定好之後，符合帳號的 App 使用者第一次登入時會自動在背景回填歷史打卡紀錄。
      </p>
    </div>

    <div
      v-if="!auth.isAdmin.value"
      class="rounded-xl border border-amber-200 bg-amber-50 p-6 text-sm text-amber-800"
    >
      只有管理員可以檢視或變更此設定。
    </div>

    <template v-else>
      <section class="rounded-xl border border-slate-200 bg-white p-6 shadow-sm space-y-5">
        <div class="grid grid-cols-1 gap-4 sm:grid-cols-2">
          <label class="text-sm sm:col-span-2">
            <span class="mb-1 block text-slate-600">
              連線字串 (mongodb://...)
              <span v-if="connectionConfigured" class="text-xs text-slate-400">（已設定；留空表示不變更）</span>
            </span>
            <input
              v-model="connectionString"
              type="password"
              autocomplete="new-password"
              :placeholder="connectionConfigured ? '●●●●●●●●' : 'mongodb://user:pass@host:27017'"
              class="w-full rounded border border-slate-300 px-2 py-1.5 font-mono"
            >
          </label>
          <label class="text-sm">
            <span class="mb-1 block text-slate-600">資料庫</span>
            <input v-model="database" type="text" class="w-full rounded border border-slate-300 px-2 py-1.5 font-mono">
          </label>
          <label class="text-sm">
            <span class="mb-1 block text-slate-600">集合 (collection)</span>
            <input v-model="collection" type="text" class="w-full rounded border border-slate-300 px-2 py-1.5 font-mono">
          </label>
          <label class="text-sm">
            <span class="mb-1 block text-slate-600">身份識別欄位（比對 App 使用者的帳號）</span>
            <input v-model="identityField" type="text" placeholder="signer.username" class="w-full rounded border border-slate-300 px-2 py-1.5 font-mono">
          </label>
          <label class="text-sm">
            <span class="mb-1 block text-slate-600">發生時間欄位</span>
            <input v-model="timestampField" type="text" placeholder="at" class="w-full rounded border border-slate-300 px-2 py-1.5 font-mono">
          </label>
          <label class="text-sm">
            <span class="mb-1 block text-slate-600">緯度欄位</span>
            <input v-model="latField" type="text" placeholder="geo.lat" class="w-full rounded border border-slate-300 px-2 py-1.5 font-mono">
          </label>
          <label class="text-sm">
            <span class="mb-1 block text-slate-600">經度欄位</span>
            <input v-model="lngField" type="text" placeholder="geo.lng" class="w-full rounded border border-slate-300 px-2 py-1.5 font-mono">
          </label>
          <label class="text-sm">
            <span class="mb-1 block text-slate-600">地址欄位（選填，對應顯示地名）</span>
            <input v-model="regionNameField" type="text" placeholder="address" class="w-full rounded border border-slate-300 px-2 py-1.5 font-mono">
          </label>
          <label class="text-sm">
            <span class="mb-1 block text-slate-600">備註欄位（選填）</span>
            <input v-model="manualLabelField" type="text" placeholder="comment" class="w-full rounded border border-slate-300 px-2 py-1.5 font-mono">
          </label>
          <label class="text-sm">
            <span class="mb-1 block text-slate-600">動作欄位</span>
            <input v-model="actionField" type="text" placeholder="action" class="w-full rounded border border-slate-300 px-2 py-1.5 font-mono">
          </label>
        </div>

        <div class="space-y-2 border-t border-slate-100 pt-4">
          <p class="text-sm font-medium text-slate-700">
            動作對應表
          </p>
          <p class="text-xs text-slate-500">
            舊系統原始動作字串沒有列在這裡的話，該筆紀錄會被跳過、不匯入。
          </p>
          <div v-for="(row, index) in actionMapRows" :key="index" class="flex items-center gap-2">
            <input
              v-model="row.source"
              type="text"
              placeholder="上班"
              class="w-32 rounded border border-slate-300 px-2 py-1.5 font-mono text-sm"
            >
            <span class="text-slate-400">→</span>
            <select v-model="row.eventType" class="flex-1 rounded border border-slate-300 px-2 py-1.5 text-sm">
              <option v-for="opt in EVENT_TYPE_OPTIONS" :key="opt.value" :value="opt.value">
                {{ opt.label }}
              </option>
            </select>
            <button
              type="button"
              class="rounded-md border border-slate-300 px-2 py-1.5 text-xs text-slate-500 hover:bg-slate-50"
              @click="removeActionRow(index)"
            >
              移除
            </button>
          </div>
          <button
            type="button"
            class="rounded-md border border-slate-300 bg-white px-3 py-1.5 text-sm text-slate-700 hover:bg-slate-50"
            @click="addActionRow"
          >
            + 新增一列
          </button>
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
            @click="save"
          >
            {{ saving ? '儲存中…' : '儲存設定' }}
          </button>
        </div>
      </section>

      <!-- Preview (config-time dry-run, no writes) -->
      <section class="rounded-xl border border-slate-200 bg-white p-6 shadow-sm space-y-4">
        <div>
          <h2 class="text-lg font-semibold text-slate-900">
            測試連線＋預覽
          </h2>
          <p class="text-sm text-slate-500">
            用目前（可能尚未儲存）的設定實際連線撈幾筆資料、套用轉換規則預覽結果。不會寫入任何資料。
          </p>
        </div>
        <div class="flex flex-wrap items-end gap-3">
          <label class="text-sm">
            <span class="mb-1 block text-slate-600">測試帳號 (username)</span>
            <input v-model="testUsername" type="text" class="rounded border border-slate-300 px-2 py-1.5 font-mono">
          </label>
          <label class="text-sm">
            <span class="mb-1 block text-slate-600">樣本筆數</span>
            <input v-model.number="previewLimit" type="number" min="1" max="50" class="w-20 rounded border border-slate-300 px-2 py-1.5">
          </label>
          <button
            type="button"
            :disabled="previewing"
            class="rounded-md border border-slate-300 bg-white px-3 py-2 text-sm font-medium text-slate-700 hover:bg-slate-50 disabled:opacity-60"
            @click="runPreview"
          >
            {{ previewing ? '測試中…' : '測試連線＋預覽' }}
          </button>
        </div>

        <div v-if="previewError" class="text-sm text-red-600">
          {{ previewError }}
        </div>
        <div
          v-else-if="previewResult"
          class="rounded-md border p-3 text-sm"
          :class="previewResult.connected ? 'border-green-200 bg-green-50' : 'border-red-200 bg-red-50'"
        >
          <template v-if="!previewResult.connected">
            <p class="font-medium text-red-700">
              連線 / 設定有問題
            </p>
            <p class="text-red-600">
              {{ previewResult.error }}
            </p>
          </template>
          <template v-else>
            <p class="font-medium text-green-700">
              ✓ 連線成功，取得 {{ previewResult.sample.length }} 筆樣本
              <span v-if="previewResult.skipped_unmapped_action || previewResult.skipped_unparseable" class="font-normal text-slate-500">
                （跳過 {{ previewResult.skipped_unmapped_action }} 筆未對應動作、{{ previewResult.skipped_unparseable }} 筆格式無法解析）
              </span>
            </p>
            <ul class="mt-2 space-y-1 font-mono text-xs text-slate-700">
              <li v-for="(e, i) in previewResult.sample" :key="i">
                {{ e.event_type }} · {{ e.occurred_at_client }} · ({{ e.lat }}, {{ e.lng }})
                <span v-if="e.region_name">· {{ e.region_name }}</span>
              </li>
            </ul>
          </template>
        </div>
      </section>

      <!-- Job status (read-only) -->
      <section class="rounded-xl border border-slate-200 bg-white p-6 shadow-sm space-y-4">
        <div class="flex items-center justify-between">
          <div>
            <h2 class="text-lg font-semibold text-slate-900">
              回填工作狀態
            </h2>
            <p class="text-sm text-slate-500">
              每位 App 使用者第一次登入後排入的回填工作。失敗達重試上限會標記為「失敗」，需要人工確認原因。
            </p>
          </div>
          <button
            type="button"
            class="rounded-md border border-slate-300 bg-white px-3 py-1.5 text-sm text-slate-700 hover:bg-slate-50"
            @click="loadJobs"
          >
            重新整理
          </button>
        </div>

        <div v-if="loadingJobs" class="text-sm text-slate-500">
          載入中...
        </div>
        <div v-else-if="jobsError" class="text-sm text-red-600">
          {{ jobsError }}
        </div>
        <div v-else-if="jobs.length === 0" class="text-sm text-slate-500">
          目前沒有回填工作。
        </div>
        <table v-else class="w-full text-left text-sm">
          <thead class="text-xs text-slate-500">
            <tr>
              <th class="pb-2">
                App 使用者
              </th>
              <th class="pb-2">
                狀態
              </th>
              <th class="pb-2">
                嘗試次數
              </th>
              <th class="pb-2">
                更新時間
              </th>
              <th class="pb-2">
                錯誤訊息
              </th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="job in jobs" :key="job.id" class="border-t border-slate-100">
              <td class="py-2 font-mono text-xs">
                {{ job.app_user_id }}
              </td>
              <td class="py-2">
                <span class="rounded border px-2 py-0.5 text-xs" :class="jobStatusClass(job.status)">
                  {{ JOB_STATUS_LABEL[job.status] ?? job.status }}
                </span>
              </td>
              <td class="py-2">
                {{ job.attempts }}
              </td>
              <td class="py-2 text-xs text-slate-500">
                {{ job.updated_at }}
              </td>
              <td class="py-2 text-xs text-red-600">
                {{ job.last_error ?? '' }}
              </td>
            </tr>
          </tbody>
        </table>
      </section>
    </template>
  </div>
</template>
