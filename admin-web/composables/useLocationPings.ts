import type { LocationListParams, LocationPingDto } from '~/types/api'

/**
 * Wraps `GET /checkin/users/:id/locations`. Newest-first by
 * `occurred_at_client`. Optional `from` / `to` for range queries (the
 * dashboard's main use case — one calendar day at a time) and `before`
 * for cursor pagination.
 */
export function useLocationPings() {
  const api = useApi()

  async function list(opts: {
    appUserId: string
    params?: LocationListParams
  }): Promise<LocationPingDto[]> {
    const query: Record<string, string> = {}
    if (opts.params?.before) query.before = opts.params.before
    if (opts.params?.limit !== undefined) query.limit = String(opts.params.limit)
    if (opts.params?.from) query.from = opts.params.from
    if (opts.params?.to) query.to = opts.params.to
    return api<LocationPingDto[]>(
      `/checkin/users/${opts.appUserId}/locations`,
      { method: 'GET', query },
    )
  }

  return { list }
}
