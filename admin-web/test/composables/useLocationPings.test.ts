import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import type { LocationPingDto } from '~/types/api'
import { useLocationPings } from '~/composables/useLocationPings'
import { useApi } from '~/composables/useApi'

vi.mock('~/composables/useApi', () => ({
  useApi: vi.fn(),
}))

const mockUseApi = vi.mocked(useApi)

describe('useLocationPings', () => {
  let captured: { url?: string, opts?: any } = {}

  beforeEach(() => {
    captured = {}
    const fakeFetch = vi.fn(async (url: string, opts: any) => {
      captured.url = url
      captured.opts = opts
      return [] as LocationPingDto[]
    })
    // Cast through unknown to satisfy the structural typing of the real
    // $fetch instance — we're only exercising the URL/query-building path.
    mockUseApi.mockReturnValue(fakeFetch as unknown as ReturnType<typeof useApi>)
  })

  afterEach(() => {
    vi.restoreAllMocks()
  })

  it('builds URL with appUserId', async () => {
    await useLocationPings().list({ appUserId: 'u1' })
    expect(captured.url).toBe('/checkin/users/u1/locations')
    expect(captured.opts.method).toBe('GET')
  })

  it('encodes from / to query params', async () => {
    await useLocationPings().list({
      appUserId: 'u1',
      params: {
        from: '2026-03-01T00:00:00+08:00',
        to: '2026-03-02T00:00:00+08:00',
      },
    })
    expect(captured.opts.query.from).toBe('2026-03-01T00:00:00+08:00')
    expect(captured.opts.query.to).toBe('2026-03-02T00:00:00+08:00')
  })

  it('encodes before cursor and limit', async () => {
    await useLocationPings().list({
      appUserId: 'u1',
      params: {
        before: '2026-03-01T05:00:00+08:00',
        limit: 50,
      },
    })
    expect(captured.opts.query.before).toBe('2026-03-01T05:00:00+08:00')
    expect(captured.opts.query.limit).toBe('50')
  })

  it('omits unset params', async () => {
    await useLocationPings().list({ appUserId: 'u1' })
    expect(captured.opts.query).toEqual({})
  })
})
