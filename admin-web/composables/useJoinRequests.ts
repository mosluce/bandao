import type {
  JoinRequestDto,
  JoinRequestStatus,
  OrgPendingJoinRequestDto,
  RejectJoinRequestRequest,
  SubmitJoinRequestRequest,
} from '~/types/api'

/**
 * Wraps the `org-join-requests` endpoints. Submitter side
 * (`/me/join-requests/*`) and admin side (`/orgs/me/join-requests/*`).
 */
export function useJoinRequests() {
  const api = useApi()

  async function submit(req: SubmitJoinRequestRequest): Promise<JoinRequestDto> {
    return api<JoinRequestDto>('/me/join-requests', {
      method: 'POST',
      body: req,
    })
  }

  async function listMine(): Promise<JoinRequestDto[]> {
    return api<JoinRequestDto[]>('/me/join-requests', { method: 'GET' })
  }

  async function cancel(id: string): Promise<void> {
    await api(`/me/join-requests/${id}`, { method: 'DELETE' })
  }

  async function listOrgPending(
    status: JoinRequestStatus = 'pending',
  ): Promise<OrgPendingJoinRequestDto[]> {
    return api<OrgPendingJoinRequestDto[]>('/orgs/me/join-requests', {
      method: 'GET',
      query: { status },
    })
  }

  async function countOrgPending(): Promise<number> {
    const list = await listOrgPending('pending')
    return list.length
  }

  async function approve(id: string): Promise<void> {
    await api(`/orgs/me/join-requests/${id}/approve`, { method: 'POST' })
  }

  async function reject(id: string, reason?: string): Promise<void> {
    const body: RejectJoinRequestRequest = {}
    if (reason && reason.length > 0) body.rejection_reason = reason
    await api(`/orgs/me/join-requests/${id}/reject`, {
      method: 'POST',
      body,
    })
  }

  return { submit, listMine, cancel, listOrgPending, countOrgPending, approve, reject }
}
