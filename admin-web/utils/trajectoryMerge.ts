// Shared trajectory point merge contract.
//
// The polyline's vertices are the day's location pings PLUS the day's checkin
// event coordinates, so the drawn line passes through the points where the
// worker actually clocked in, transferred, and clocked out. Without the merge
// those markers float off the path by construction: the tracker only starts
// after the server confirms the clock-in, stops the moment 下班 is tapped, and
// samples on a 100m AND 60s filter that never lands exactly on an event.
//
// This is a contract shared verbatim with the app
// (`app/lib/features/trajectory/data/trajectory_merge.dart`) — the source
// filter, the three-level ordering, and the instant-based comparison MUST
// match so both surfaces draw the same line. See openspec capability
// `app-personal-trajectory`, "Trajectory point merge contract".

import type { CheckinEventDto, LocationPingDto } from '~/types/api'

/** Origin rank — an event sorts before a ping at the same instant. */
export const ORIGIN_EVENT = 0
export const ORIGIN_PING = 1

export interface MergedPoint {
  lat: number
  lng: number
  occurredAtClient: string
  /** `ORIGIN_EVENT` or `ORIGIN_PING`; the second sort level. */
  originRank: number
  /** ObjectId hex; the final sort level, making the order total. */
  id: string
}

/**
 * Build the ordered vertex list for one day.
 *
 * Events with `source === 'admin_force'` are excluded: force-checkout copies
 * its location from the AppUser's *previous* event rather than capturing a
 * position, so a vertex there would draw a leg from wherever the worker
 * actually was back to a stale coordinate — the one case where merging makes
 * the picture actively wrong rather than merely coarse. `app` and
 * `legacy_backfill` both carry genuine captured coordinates and are included.
 *
 * Excluding an event from the series does NOT exclude it from the markers;
 * callers keep drawing markers from the full event list.
 */
export function buildMergedSeries(
  pings: LocationPingDto[],
  events: CheckinEventDto[],
): MergedPoint[] {
  const points: MergedPoint[] = []

  for (const p of pings) {
    points.push({
      lat: p.lat,
      lng: p.lng,
      occurredAtClient: p.occurred_at_client,
      originRank: ORIGIN_PING,
      id: p.id,
    })
  }

  for (const e of events) {
    if (e.source === 'admin_force') continue
    const c = e.location?.coordinates
    if (!c) continue
    points.push({
      lat: c.lat,
      lng: c.lng,
      occurredAtClient: e.occurred_at_client,
      originRank: ORIGIN_EVENT,
      id: e.id,
    })
  }

  // Compare parsed instants, never raw strings. The pings and the events come
  // from two independent serialisation sites; both emit UTC today, but a
  // lexical compare would silently mis-order the merged series the moment one
  // of them emitted an offset like `+08:00` instead.
  return points.sort((a, b) => {
    const at = Date.parse(a.occurredAtClient)
    const bt = Date.parse(b.occurredAtClient)
    if (at !== bt) return at - bt
    if (a.originRank !== b.originRank) return a.originRank - b.originRank
    return a.id < b.id ? -1 : a.id > b.id ? 1 : 0
  })
}
