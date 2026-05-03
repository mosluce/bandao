import type {
  CheckinEventDto,
  CheckinUserBoardRowDto,
  ForceCheckoutRequest,
} from '~/types/api'

/**
 * Wraps the admin-side `/checkin/*` endpoints. All calls require an
 * authenticated dashboard admin in `current_org`.
 */
export function useCheckin() {
  const api = useApi()

  async function listUsers(): Promise<CheckinUserBoardRowDto[]> {
    return api<CheckinUserBoardRowDto[]>('/checkin/users', { method: 'GET' })
  }

  async function listUserEvents(
    appUserId: string,
    opts: { before?: string, limit?: number } = {},
  ): Promise<CheckinEventDto[]> {
    const params = new URLSearchParams()
    if (opts.before) params.set('before', opts.before)
    if (opts.limit !== undefined) params.set('limit', String(opts.limit))
    const qs = params.toString()
    const path = qs ? `/checkin/users/${appUserId}/events?${qs}` : `/checkin/users/${appUserId}/events`
    return api<CheckinEventDto[]>(path, { method: 'GET' })
  }

  async function forceCheckout(
    appUserId: string,
    reason?: string,
  ): Promise<CheckinEventDto> {
    const body: ForceCheckoutRequest = reason ? { reason } : {}
    return api<CheckinEventDto>(`/checkin/users/${appUserId}/force-checkout`, {
      method: 'POST',
      body,
    })
  }

  return { listUsers, listUserEvents, forceCheckout }
}
