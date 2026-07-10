// Shared time-of-day trajectory color scale.
//
// Maps a local wall-clock time to a color on a two-pole warm→cool ramp:
// 06:00 warmest → 22:00 coolest, clamped outside that window. The ramp runs
// through the red–purple side (never green/rainbow) and stays chromatic
// throughout so the path is legible on the light CARTO Positron basemap.
//
// This is a contract shared verbatim with the app
// (`app/lib/features/trajectory/data/time_of_day_color.dart`) — the anchors,
// domain, clamp, and linear-RGB interpolation MUST match so both surfaces
// render identically. See openspec capability `app-personal-trajectory`.

interface Rgb { r: number, g: number, b: number }

// (minuteOfDay, [r,g,b]) anchors. minuteOfDay = hour*60 + minute.
const ANCHORS: [number, Rgb][] = [
  [6 * 60, { r: 0xEA, g: 0x58, b: 0x0C }], // 06:00 orange (warmest)
  [10 * 60, { r: 0xE1, g: 0x1D, b: 0x48 }], // 10:00 rose
  [14 * 60, { r: 0xC0, g: 0x26, b: 0xD3 }], // 14:00 fuchsia (bridge)
  [18 * 60, { r: 0x7C, g: 0x3A, b: 0xED }], // 18:00 violet
  [22 * 60, { r: 0x43, g: 0x38, b: 0xCA }], // 22:00 indigo (coolest)
]

function toHex(c: Rgb): string {
  const h = (n: number) => Math.round(n).toString(16).padStart(2, '0')
  return `#${h(c.r)}${h(c.g)}${h(c.b)}`
}

/** Color (hex) for a `minuteOfDay` (`hour*60 + minute`), clamped to the domain. */
export function timeOfDayColorForMinute(minuteOfDay: number): string {
  const [firstM, firstC] = ANCHORS[0]
  const [lastM, lastC] = ANCHORS[ANCHORS.length - 1]
  if (minuteOfDay <= firstM) return toHex(firstC)
  if (minuteOfDay >= lastM) return toHex(lastC)

  for (let i = 0; i < ANCHORS.length - 1; i++) {
    const [loM, loC] = ANCHORS[i]
    const [hiM, hiC] = ANCHORS[i + 1]
    if (minuteOfDay >= loM && minuteOfDay <= hiM) {
      const t = (minuteOfDay - loM) / (hiM - loM)
      return toHex({
        r: loC.r + (hiC.r - loC.r) * t,
        g: loC.g + (hiC.g - loC.g) * t,
        b: loC.b + (hiC.b - loC.b) * t,
      })
    }
  }
  return toHex(lastC)
}

/**
 * Minute-of-day (`hour*60 + minute`) of an ISO timestamp read in `tz`.
 * Used so path colors reflect the employee's wall clock in the Org timezone.
 */
export function minuteOfDayInTz(iso: string, tz: string): number {
  const parts = new Intl.DateTimeFormat('en-GB', {
    timeZone: tz,
    hour: '2-digit',
    minute: '2-digit',
    hour12: false,
  }).formatToParts(new Date(iso))
  const hour = Number(parts.find(p => p.type === 'hour')?.value ?? '0')
  const minute = Number(parts.find(p => p.type === 'minute')?.value ?? '0')
  return hour * 60 + minute
}
