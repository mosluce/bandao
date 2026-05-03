/**
 * Format an absolute timestamp under the active Org's timezone. Falls back
 * to the browser's locale if the Org timezone is missing or invalid. The
 * timezone argument is plumbed through useAuth().currentOrg.value?.timezone
 * (after add-checkin-events lands the field).
 */
export function formatInOrgTz(iso: string | null | undefined, timezone?: string | null): string {
  if (!iso) return '—'
  try {
    const d = new Date(iso)
    if (timezone) {
      return d.toLocaleString('zh-Hant', { timeZone: timezone, hour12: false })
    }
    return d.toLocaleString('zh-Hant', { hour12: false })
  }
  catch {
    return iso
  }
}

/**
 * Compute "X 時 Y 分" elapsed since `iso`, or null when iso is missing.
 * Used to show shift duration on the live board.
 */
export function shiftDuration(iso: string | null | undefined): string | null {
  if (!iso) return null
  try {
    const start = new Date(iso).getTime()
    const now = Date.now()
    const minutes = Math.max(0, Math.floor((now - start) / 60000))
    const h = Math.floor(minutes / 60)
    const m = minutes % 60
    if (h > 0) return `${h} 時 ${m} 分`
    return `${m} 分`
  }
  catch {
    return null
  }
}
