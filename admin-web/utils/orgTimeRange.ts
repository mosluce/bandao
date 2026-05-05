/**
 * Convert a calendar date (`YYYY-MM-DD`) plus an IANA timezone into the
 * RFC3339 instants that bracket that day in the given zone. Honors DST —
 * resolves the offset at midnight on the requested date for both the start
 * and end of the day, so 23-hour and 25-hour days produce sensible ranges.
 *
 * Examples:
 *   dateToOrgRange('2026-03-01', 'Asia/Taipei')
 *     → { from: '2026-03-01T00:00:00+08:00',
 *         to:   '2026-03-02T00:00:00+08:00' }
 *
 *   dateToOrgRange('2026-03-09', 'America/Los_Angeles')   // DST springs forward
 *     → { from: '2026-03-09T00:00:00-08:00',
 *         to:   '2026-03-10T00:00:00-07:00' }
 */
export function dateToOrgRange(date: string, timezone: string): { from: string, to: string } {
  if (!/^\d{4}-\d{2}-\d{2}$/.test(date)) {
    throw new Error(`dateToOrgRange: invalid date format ${date}`)
  }
  const startOffset = offsetAtMidnight(date, timezone)
  const next = nextDate(date)
  const endOffset = offsetAtMidnight(next, timezone)
  return {
    from: `${date}T00:00:00${startOffset}`,
    to: `${next}T00:00:00${endOffset}`,
  }
}

function nextDate(date: string): string {
  const [y, m, d] = date.split('-').map(Number)
  // Use UTC arithmetic to avoid local-tz pitfalls in the rollover calc.
  const t = Date.UTC(y, m - 1, d) + 86_400_000
  const next = new Date(t)
  return `${next.getUTCFullYear()}-${pad(next.getUTCMonth() + 1)}-${pad(next.getUTCDate())}`
}

function pad(n: number): string {
  return n < 10 ? `0${n}` : String(n)
}

/**
 * Compute the wall-clock-to-UTC offset for the given IANA timezone at
 * `date T00:00`. Returns a string formatted `+HH:MM` / `-HH:MM`.
 *
 * Strategy: ask Intl.DateTimeFormat for the parts of `date T00:00` *as if*
 * the wall clock were UTC, then compare with what the same instant looks
 * like in the target timezone. The signed difference is the offset.
 */
function offsetAtMidnight(date: string, timezone: string): string {
  const [y, m, d] = date.split('-').map(Number)
  // Probe instant: midnight in UTC on the given date.
  const probe = Date.UTC(y, m - 1, d)
  // Format the probe in the target timezone — what wall time does it land on?
  const fmt = new Intl.DateTimeFormat('en-CA', {
    timeZone: timezone,
    year: 'numeric',
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit',
    hour12: false,
  })
  const parts = Object.fromEntries(
    fmt.formatToParts(new Date(probe))
      .filter(p => p.type !== 'literal')
      .map(p => [p.type, p.value]),
  )
  const wall = Date.UTC(
    Number(parts.year),
    Number(parts.month) - 1,
    Number(parts.day),
    Number(parts.hour === '24' ? '00' : parts.hour),
    Number(parts.minute),
    Number(parts.second),
  )
  // wall − UTC = offset (in ms). DST transitions can land us before/after
  // the requested date in this representation; iterate up to a few hours
  // until the wall date matches the requested date so we resolve "midnight
  // of <date>" rather than some neighbouring instant.
  let offsetMs = wall - probe
  // Solve f(t) = offset(probe + offsetMs) recursively — converges in 1–2
  // iterations except at DST boundaries, which still settle within ~3.
  for (let i = 0; i < 4; i++) {
    const candidate = probe - offsetMs
    const cParts = Object.fromEntries(
      fmt.formatToParts(new Date(candidate))
        .filter(p => p.type !== 'literal')
        .map(p => [p.type, p.value]),
    )
    if (
      cParts.year === String(y)
      && cParts.month === pad(m)
      && cParts.day === pad(d)
      && cParts.hour === '00'
    ) {
      break
    }
    const cWall = Date.UTC(
      Number(cParts.year),
      Number(cParts.month) - 1,
      Number(cParts.day),
      Number(cParts.hour === '24' ? '00' : cParts.hour),
      Number(cParts.minute),
      Number(cParts.second),
    )
    offsetMs = cWall - candidate
  }
  return formatOffset(offsetMs)
}

function formatOffset(offsetMs: number): string {
  const sign = offsetMs >= 0 ? '+' : '-'
  const abs = Math.abs(offsetMs)
  const hh = pad(Math.floor(abs / 3_600_000))
  const mm = pad(Math.floor((abs % 3_600_000) / 60_000))
  return `${sign}${hh}:${mm}`
}
