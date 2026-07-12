import type {
  ApiTokenDto,
  ApiTokenSecretResponse,
  CreateApiTokenRequest,
  UpdateApiTokenStatusRequest,
} from '~/types/api'

/**
 * Wraps the admin-side `/orgs/me/api-tokens/*` endpoints. Requires an
 * authenticated dashboard admin in `current_org`. `create` and `rotate` are
 * the only calls that ever see the plaintext secret — callers must not
 * persist it beyond the one-time-reveal UI.
 */
export function useApiTokens() {
  const api = useApi()

  async function list(): Promise<ApiTokenDto[]> {
    return api<ApiTokenDto[]>('/orgs/me/api-tokens', { method: 'GET' })
  }

  async function create(req: CreateApiTokenRequest): Promise<ApiTokenSecretResponse> {
    return api<ApiTokenSecretResponse>('/orgs/me/api-tokens', {
      method: 'POST',
      body: req,
    })
  }

  async function rotate(id: string): Promise<ApiTokenSecretResponse> {
    return api<ApiTokenSecretResponse>(`/orgs/me/api-tokens/${id}/rotate`, {
      method: 'POST',
    })
  }

  async function updateStatus(id: string, req: UpdateApiTokenStatusRequest): Promise<ApiTokenDto> {
    return api<ApiTokenDto>(`/orgs/me/api-tokens/${id}`, {
      method: 'PATCH',
      body: req,
    })
  }

  async function disable(id: string): Promise<ApiTokenDto> {
    return updateStatus(id, { status: 'disabled' })
  }

  async function enable(id: string): Promise<ApiTokenDto> {
    return updateStatus(id, { status: 'active' })
  }

  async function remove(id: string): Promise<void> {
    await api<void>(`/orgs/me/api-tokens/${id}`, { method: 'DELETE' })
  }

  return { list, create, rotate, disable, enable, remove }
}
