import { describe, expect, it } from 'vitest'

import { dateToOrgRange } from '~/utils/orgTimeRange'

describe('dateToOrgRange', () => {
  it('handles Asia/Taipei (no DST, +08:00)', () => {
    const r = dateToOrgRange('2026-03-01', 'Asia/Taipei')
    expect(r.from).toBe('2026-03-01T00:00:00+08:00')
    expect(r.to).toBe('2026-03-02T00:00:00+08:00')
  })

  it('handles month rollover', () => {
    const r = dateToOrgRange('2026-01-31', 'Asia/Taipei')
    expect(r.to).toBe('2026-02-01T00:00:00+08:00')
  })

  it('handles year rollover', () => {
    const r = dateToOrgRange('2026-12-31', 'Asia/Taipei')
    expect(r.to).toBe('2027-01-01T00:00:00+08:00')
  })

  it('handles America/Los_Angeles spring-forward (23-hour day)', () => {
    // 2026 US DST starts on 2026-03-08; America/Los_Angeles is -08:00 before
    // and -07:00 after. The day itself starts at -08:00 and the next day's
    // midnight is -07:00.
    const r = dateToOrgRange('2026-03-08', 'America/Los_Angeles')
    expect(r.from).toBe('2026-03-08T00:00:00-08:00')
    expect(r.to).toBe('2026-03-09T00:00:00-07:00')
  })

  it('handles UTC', () => {
    const r = dateToOrgRange('2026-05-05', 'UTC')
    expect(r.from).toBe('2026-05-05T00:00:00+00:00')
    expect(r.to).toBe('2026-05-06T00:00:00+00:00')
  })

  it('rejects invalid date format', () => {
    expect(() => dateToOrgRange('2026/05/05', 'Asia/Taipei')).toThrow()
    expect(() => dateToOrgRange('not-a-date', 'Asia/Taipei')).toThrow()
  })
})
