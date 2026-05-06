<script setup lang="ts">
// Public privacy policy page. No middleware applied — the route stays
// reachable for authenticated users, unauthenticated users, and external
// webview / browser launches alike. The Flutter app's location-tracking
// consent dialog will link to this URL.
//
// Bump LAST_UPDATED_AT only when the policy text actually changes — NOT
// on routine redeploys. The date should reflect when the policy itself
// was edited, not the bundle build time.
const LAST_UPDATED_AT = '2026-05-04'
const PLATFORM_CONTACT_EMAIL = 'noreply@example.com'
</script>

<template>
  <main class="min-h-screen px-4 py-10">
    <div class="max-w-3xl mx-auto space-y-6">
      <header>
        <h1 class="text-2xl font-semibold text-slate-900">
          隱私政策
        </h1>
        <p class="mt-1 text-sm text-slate-500">
          班到平台關於資料蒐集、使用與保留的說明。
        </p>
      </header>

      <section class="rounded-xl border border-slate-200 bg-white p-6 shadow-sm space-y-3">
        <h2 class="text-lg font-semibold text-slate-900">
          1. 適用範圍
        </h2>
        <p class="text-sm text-slate-700">
          本政策說明 班到平台（以下稱「本平台」）以及您所屬組織（以下稱「Org」）使用 班到服務時的資料處理方式。組織管理員可能依其內部流程訂定額外的資料蒐集與使用規則，請另向您的 Org 確認。
        </p>
      </section>

      <section class="rounded-xl border border-slate-200 bg-white p-6 shadow-sm space-y-3">
        <h2 class="text-lg font-semibold text-slate-900">
          2. 我們蒐集的資料
        </h2>
        <ul class="list-disc list-outside pl-6 space-y-2 text-sm text-slate-700">
          <li>
            <span class="font-medium">帳號識別資料</span>：您建立帳號時提供的 email、使用者名稱、顯示名稱，以及經雜湊（hash）後的密碼。我們不會以明文儲存您的密碼。
          </li>
          <li>
            <span class="font-medium">出勤事件</span>：您於 班到 App 提交的打卡事件（上班、下班、轉出、轉入），含按下按鈕當下的座標、可選的 GPS 精度，以及由本平台自動執行的反向地理編碼結果（如「信義區 · 忠孝東路五段」這類包含區域與街道的名稱）。本平台僅收集到街道層級，不包含巷弄與門牌號碼。
          </li>
          <li>
            <span class="font-medium">位置軌跡</span>：若您所屬組織開啟「定位追蹤」功能，工作期間您的裝置會每分鐘記錄一次位置（同時須移動超過 100 公尺才會儲存），用以協助雇主了解現場狀況。下班後不再記錄。
          </li>
          <li>
            <span class="font-medium">技術資料</span>：登入 session、操作時間戳記、為了維持平台穩定運作所需的最小化日誌資訊。
          </li>
        </ul>
      </section>

      <section class="rounded-xl border border-slate-200 bg-white p-6 shadow-sm space-y-3">
        <h2 class="text-lg font-semibold text-slate-900">
          3. 蒐集目的
        </h2>
        <ul class="list-disc list-outside pl-6 space-y-2 text-sm text-slate-700">
          <li>履行《勞動基準法》第 30 條第 5 項所定的雇主出勤紀錄義務。</li>
          <li>協助您所屬組織確認員工的工作現場、現場安全與糾紛佐證。</li>
          <li>維持帳號安全、防止濫用、進行系統運維與故障排除。</li>
        </ul>
      </section>

      <section class="rounded-xl border border-slate-200 bg-white p-6 shadow-sm space-y-3">
        <h2 class="text-lg font-semibold text-slate-900">
          4. 保留期間
        </h2>
        <dl class="space-y-2 text-sm text-slate-700">
          <div class="flex flex-wrap gap-x-3">
            <dt class="font-medium">出勤事件</dt>
            <dd>5 年（依《勞動基準法》第 30 條第 5 項）。</dd>
          </div>
          <div class="flex flex-wrap gap-x-3">
            <dt class="font-medium">位置軌跡</dt>
            <dd>90 天，到期後由系統自動清除。</dd>
          </div>
          <div class="flex flex-wrap gap-x-3">
            <dt class="font-medium">帳號識別資料</dt>
            <dd>帳號有效期間。帳號刪除後依法令保留期限後清除。</dd>
          </div>
        </dl>
      </section>

      <section class="rounded-xl border border-slate-200 bg-white p-6 shadow-sm space-y-3">
        <h2 class="text-lg font-semibold text-slate-900">
          5. 誰能存取您的資料
        </h2>
        <ul class="list-disc list-outside pl-6 space-y-2 text-sm text-slate-700">
          <li>您所屬組織的 admin 與 owner（依組織內角色分工存取）。</li>
          <li>班到平台的維運人員，僅在系統管理、故障排除、安全事件處理等必要範圍內存取。</li>
          <li>除上述情形外，本平台不會將您的個人資料提供予任何第三方，亦不會用於行銷目的。</li>
        </ul>
      </section>

      <section class="rounded-xl border border-slate-200 bg-white p-6 shadow-sm space-y-3">
        <h2 class="text-lg font-semibold text-slate-900">
          6. 您的權利
        </h2>
        <p class="text-sm text-slate-700">
          依《個人資料保護法》第 3 條、第 10 條、第 11 條，您對於您的個人資料享有下列權利：
        </p>
        <ul class="list-disc list-outside pl-6 space-y-2 text-sm text-slate-700">
          <li>查閱本平台保有與您有關的個人資料。</li>
          <li>請求更正錯誤的資料。</li>
          <li>請求停止蒐集、處理、利用，或請求刪除（依法令義務保留期間內者除外）。</li>
          <li>撤回先前提供之同意；惟撤回不影響撤回前已合法蒐集處理之資料。</li>
        </ul>
        <p class="text-sm text-slate-700">
          行使方式：請聯絡您所屬組織的 admin。組織端會核實身分後處理或轉介至本平台維運。
        </p>
      </section>

      <section class="rounded-xl border border-slate-200 bg-white p-6 shadow-sm space-y-3">
        <h2 class="text-lg font-semibold text-slate-900">
          7. Cookie 與 Session
        </h2>
        <p class="text-sm text-slate-700">
          本平台僅使用維持登入狀態所必要的 session cookie。我們不使用任何追蹤、廣告或第三方分析 cookies。
        </p>
      </section>

      <section class="rounded-xl border border-slate-200 bg-white p-6 shadow-sm space-y-3">
        <h2 class="text-lg font-semibold text-slate-900">
          8. 聯絡方式
        </h2>
        <p class="text-sm text-slate-700">
          有關您所屬組織內部的資料處理疑義，請逕洽該組織的 admin 或 owner。
        </p>
        <p class="text-sm text-slate-700">
          有關 班到平台層級的疑義，請聯絡：
          <code class="rounded bg-slate-100 px-2 py-1 font-mono text-slate-900">{{ PLATFORM_CONTACT_EMAIL }}</code>
          <span class="text-xs text-slate-500">（placeholder — 待平台運營更新）</span>
        </p>
      </section>

      <section class="rounded-xl border border-slate-200 bg-white p-6 shadow-sm space-y-3">
        <h2 class="text-lg font-semibold text-slate-900">
          9. 政策更新
        </h2>
        <p class="text-sm text-slate-700">
          本政策若有重大變更，本平台將於變更生效前 30 天透過 admin-web 介面或其他適當管道通知您。
        </p>
        <p class="text-sm text-slate-500">
          最後更新日期：{{ LAST_UPDATED_AT }}
        </p>
      </section>

      <footer class="rounded-xl border border-amber-200 bg-amber-50 p-4 text-sm text-amber-900">
        本政策範本未經法律審查，建議您所屬組織自行確認符合當地法規。
      </footer>
    </div>
  </main>
</template>
