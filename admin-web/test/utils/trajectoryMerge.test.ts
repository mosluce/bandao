import { describe, expect, it } from 'vitest'

import type { CheckinEventDto, EventSource, LocationPingDto } from '~/types/api'
import { ORIGIN_EVENT, ORIGIN_PING, buildMergedSeries } from '~/utils/trajectoryMerge'

// These fixtures are mirrored by the Flutter suite in
// `app/test/features/trajectory/trajectory_merge_test.dart`. Both platforms
// must agree on every case below — that agreement IS the contract.

function ping(id: string, iso: string, lat = 25, lng = 121): LocationPingDto {
  return {
    id,
    app_user_id: 'u1',
    lat,
    lng,
    occurred_at_client: iso,
    occurred_at_server: iso,
  }
}

function event(
  id: string,
  iso: string,
  opts: { lat?: number, lng?: number, source?: EventSource } = {},
): CheckinEventDto {
  return {
    id,
    app_user_id: 'u1',
    event_type: 'clock_in',
    occurred_at_client: iso,
    occurred_at_server: iso,
    source: opts.source ?? 'app',
    initiated_by_kind: 'app_user',
    initiated_by_id: 'u1',
    location: {
      coordinates: { lat: opts.lat ?? 25, lng: opts.lng ?? 121 },
    },
    has_skew_warning: false,
  }
}

describe('buildMergedSeries', () => {
  it('interleaves pings and events by time', () => {
    const pings = [
      ping('p1', '2026-03-01T08:04:00Z'),
      ping('p2', '2026-03-01T08:12:00Z'),
      ping('p3', '2026-03-01T12:33:00Z'),
      ping('p4', '2026-03-01T17:52:00Z'),
    ]
    const events = [
      event('e1', '2026-03-01T08:00:00Z'),
      event('e2', '2026-03-01T12:30:00Z'),
      event('e3', '2026-03-01T17:55:00Z'),
    ]

    const merged = buildMergedSeries(pings, events)

    expect(merged.map(p => p.id)).toEqual([
      'e1',
      'p1',
      'p2',
      'e2',
      'p3',
      'p4',
      'e3',
    ])
  })

  it('puts the clock-in first and the clock-out last', () => {
    const merged = buildMergedSeries(
      [ping('p1', '2026-03-01T09:00:00Z', 25.1, 121.1)],
      [
        event('in', '2026-03-01T08:00:00Z', { lat: 25.0, lng: 121.0 }),
        event('out', '2026-03-01T17:00:00Z', { lat: 25.9, lng: 121.9 }),
      ],
    )

    expect(merged[0].id).toBe('in')
    expect(merged[0].lat).toBe(25.0)
    expect(merged[merged.length - 1].id).toBe('out')
    expect(merged[merged.length - 1].lat).toBe(25.9)
  })

  it('excludes admin_force events (location is copied, not captured)', () => {
    const merged = buildMergedSeries(
      [ping('p1', '2026-03-01T09:00:00Z')],
      [
        event('in', '2026-03-01T08:00:00Z'),
        event('forced', '2026-03-01T23:00:00Z', { source: 'admin_force' }),
      ],
    )

    expect(merged.map(p => p.id)).toEqual(['in', 'p1'])
  })

  it('includes legacy_backfill events (coordinates are genuine)', () => {
    const merged = buildMergedSeries(
      [],
      [
        event('l1', '2025-08-01T08:00:00Z', { source: 'legacy_backfill' }),
        event('l2', '2025-08-01T17:00:00Z', { source: 'legacy_backfill' }),
      ],
    )

    expect(merged.map(p => p.id)).toEqual(['l1', 'l2'])
  })

  it('breaks an equal-timestamp tie toward the event', () => {
    const merged = buildMergedSeries(
      [ping('p1', '2026-03-01T08:00:00Z')],
      [event('e1', '2026-03-01T08:00:00Z')],
    )

    expect(merged.map(p => p.id)).toEqual(['e1', 'p1'])
    expect(merged[0].originRank).toBe(ORIGIN_EVENT)
    expect(merged[1].originRank).toBe(ORIGIN_PING)
  })

  it('breaks a same-instant same-origin tie by id, making the order total', () => {
    const merged = buildMergedSeries(
      [ping('pb', '2026-03-01T08:00:00Z'), ping('pa', '2026-03-01T08:00:00Z')],
      [],
    )

    expect(merged.map(p => p.id)).toEqual(['pa', 'pb'])
  })

  it('orders by instant, not by raw string, across mixed offsets', () => {
    // 08:30+08:00 === 00:30Z, which is EARLIER than 01:00Z even though the
    // raw string sorts later. A lexical compare would invert these.
    const merged = buildMergedSeries(
      [ping('pingAt0100Z', '2026-03-01T01:00:00Z')],
      [event('eventAt0830Plus8', '2026-03-01T08:30:00+08:00')],
    )

    expect(merged.map(p => p.id)).toEqual(['eventAt0830Plus8', 'pingAt0100Z'])
  })

  it('keeps near-coincident points rather than deduplicating', () => {
    const merged = buildMergedSeries(
      [ping('p1', '2026-03-01T17:54:57Z', 25.000, 121.000)],
      [event('out', '2026-03-01T17:55:00Z', { lat: 25.00002, lng: 121.00002 })],
    )

    expect(merged).toHaveLength(2)
    expect(merged.map(p => p.id)).toEqual(['p1', 'out'])
  })

  it('returns an empty series for no input', () => {
    expect(buildMergedSeries([], [])).toEqual([])
  })

  it('handles pings only and events only', () => {
    expect(buildMergedSeries([ping('p1', '2026-03-01T08:00:00Z')], [])).toHaveLength(1)
    expect(buildMergedSeries([], [event('e1', '2026-03-01T08:00:00Z')])).toHaveLength(1)
  })
})
