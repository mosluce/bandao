import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import { useJoinRequests } from '~/composables/useJoinRequests'
import { useApi } from '~/composables/useApi'

vi.mock('~/composables/useApi', () => ({
  useApi: vi.fn(),
}))

const mockUseApi = vi.mocked(useApi)

describe('useJoinRequests', () => {
  let captured: { url?: string, opts?: any } = {}

  beforeEach(() => {
    captured = {}
    const fakeFetch = vi.fn(async (url: string, opts: any) => {
      captured.url = url
      captured.opts = opts
      // Return shapes appropriate for each call site under test.
      return [] as unknown
    })
    mockUseApi.mockReturnValue(fakeFetch as unknown as ReturnType<typeof useApi>)
  })

  afterEach(() => {
    vi.restoreAllMocks()
  })

  it('submit POSTs to /me/join-requests with body', async () => {
    await useJoinRequests().submit({
      org_code: 'ABCDEFGHIJ',
      application_message: 'hi',
    })
    expect(captured.url).toBe('/me/join-requests')
    expect(captured.opts.method).toBe('POST')
    expect(captured.opts.body).toEqual({
      org_code: 'ABCDEFGHIJ',
      application_message: 'hi',
    })
  })

  it('listMine GETs /me/join-requests', async () => {
    await useJoinRequests().listMine()
    expect(captured.url).toBe('/me/join-requests')
    expect(captured.opts.method).toBe('GET')
  })

  it('cancel DELETEs /me/join-requests/:id', async () => {
    await useJoinRequests().cancel('req-123')
    expect(captured.url).toBe('/me/join-requests/req-123')
    expect(captured.opts.method).toBe('DELETE')
  })

  it('listOrgPending defaults to status=pending', async () => {
    await useJoinRequests().listOrgPending()
    expect(captured.url).toBe('/orgs/me/join-requests')
    expect(captured.opts.query.status).toBe('pending')
  })

  it('listOrgPending honors explicit status', async () => {
    await useJoinRequests().listOrgPending('rejected')
    expect(captured.opts.query.status).toBe('rejected')
  })

  it('approve POSTs to /orgs/me/join-requests/:id/approve', async () => {
    await useJoinRequests().approve('abc')
    expect(captured.url).toBe('/orgs/me/join-requests/abc/approve')
    expect(captured.opts.method).toBe('POST')
  })

  it('reject without reason omits body field', async () => {
    await useJoinRequests().reject('abc')
    expect(captured.url).toBe('/orgs/me/join-requests/abc/reject')
    expect(captured.opts.method).toBe('POST')
    expect(captured.opts.body).toEqual({})
  })

  it('reject with reason includes rejection_reason', async () => {
    await useJoinRequests().reject('abc', '不收外部承包商')
    expect(captured.opts.body.rejection_reason).toBe('不收外部承包商')
  })
})
