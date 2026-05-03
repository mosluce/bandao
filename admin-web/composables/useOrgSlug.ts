import type { SetSlugRequest, SetSlugResponse } from '~/types/api'

export function useOrgSlug() {
  const api = useApi()

  async function setOrgSlug(slug: string): Promise<SetSlugResponse> {
    return await api<SetSlugResponse>('/orgs/me/slug', {
      method: 'POST',
      body: { slug } satisfies SetSlugRequest,
    })
  }

  async function clearOrgSlug(): Promise<void> {
    await api('/orgs/me/slug', { method: 'DELETE' })
  }

  return { setOrgSlug, clearOrgSlug }
}
