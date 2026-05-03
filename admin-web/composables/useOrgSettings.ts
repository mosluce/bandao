import type { OrgSettingsDto, UpdateOrgSettingsRequest } from '~/types/api'

/**
 * Wraps `PATCH /orgs/me/settings`. transfer_enabled is state-locked
 * (rejected with STATE_LOCKED while anyone is on shift); timezone is
 * always changeable.
 */
export function useOrgSettings() {
  const api = useApi()

  async function update(req: UpdateOrgSettingsRequest): Promise<OrgSettingsDto> {
    return api<OrgSettingsDto>('/orgs/me/settings', {
      method: 'PATCH',
      body: req,
    })
  }

  return { update }
}
