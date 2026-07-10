import { describe, expect, it } from 'vitest'

import { minuteOfDayInTz, timeOfDayColorForMinute } from '~/utils/timeOfDayColor'

describe('timeOfDayColorForMinute', () => {
  it('anchors map to their exact hex (must match the Flutter app scale)', () => {
    expect(timeOfDayColorForMinute(6 * 60)).toBe('#ea580c')
    expect(timeOfDayColorForMinute(14 * 60)).toBe('#c026d3')
    expect(timeOfDayColorForMinute(22 * 60)).toBe('#4338ca')
  })

  it('clamps below 06:00 and above 22:00', () => {
    expect(timeOfDayColorForMinute(0)).toBe('#ea580c')
    expect(timeOfDayColorForMinute(5 * 60 + 30)).toBe('#ea580c')
    expect(timeOfDayColorForMinute(23 * 60 + 15)).toBe('#4338ca')
    expect(timeOfDayColorForMinute(24 * 60)).toBe('#4338ca')
  })

  it('interpolates midway between two anchors (08:00 halfway 06:00–10:00)', () => {
    // r: 0xEA→0xE1, g: 0x58→0x1D, b: 0x0C→0x48 at t=0.5
    const r = Math.round((0xEA + 0xE1) / 2)
    const g = Math.round((0x58 + 0x1D) / 2)
    const b = Math.round((0x0C + 0x48) / 2)
    const hex = `#${r.toString(16)}${g.toString(16)}${b.toString(16)}`
    expect(timeOfDayColorForMinute(8 * 60)).toBe(hex)
  })

  it('produces five distinct anchor colors', () => {
    const set = new Set([6, 10, 14, 18, 22].map(h => timeOfDayColorForMinute(h * 60)))
    expect(set.size).toBe(5)
  })
})

describe('minuteOfDayInTz', () => {
  it('reads the wall-clock hour/minute in the given timezone', () => {
    // 2026-07-10T06:30:00+08:00 is 06:30 in Asia/Taipei → 390 minutes.
    expect(minuteOfDayInTz('2026-07-10T06:30:00+08:00', 'Asia/Taipei')).toBe(390)
  })

  it('respects the target timezone regardless of the offset in the string', () => {
    // Same instant expressed in UTC; in Asia/Taipei it is 06:30 → 390.
    expect(minuteOfDayInTz('2026-07-09T22:30:00Z', 'Asia/Taipei')).toBe(390)
  })
})
