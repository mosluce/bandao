import type {
  AppUserDto,
  CreateAppUserRequest,
  CreateAppUserResponse,
  PasswordResetResponse,
  UpdateAppUserRequest,
} from '~/types/api'

/**
 * Wraps the admin-side `/app-users/*` endpoints. All calls require an
 * authenticated dashboard admin in `current_org`; non-admin or zero-Org
 * sessions are surfaced via the existing `useApi` ApiError flow.
 */
export function useAppUsers() {
  const api = useApi()

  async function list(): Promise<AppUserDto[]> {
    return api<AppUserDto[]>('/app-users', { method: 'GET' })
  }

  async function create(req: CreateAppUserRequest): Promise<CreateAppUserResponse> {
    return api<CreateAppUserResponse>('/app-users', {
      method: 'POST',
      body: req,
    })
  }

  async function update(id: string, req: UpdateAppUserRequest): Promise<AppUserDto> {
    return api<AppUserDto>(`/app-users/${id}`, {
      method: 'PATCH',
      body: req,
    })
  }

  async function disable(id: string): Promise<AppUserDto> {
    return update(id, { status: 'disabled' })
  }

  async function enable(id: string): Promise<AppUserDto> {
    return update(id, { status: 'active' })
  }

  async function resetPassword(id: string): Promise<PasswordResetResponse> {
    return api<PasswordResetResponse>(`/app-users/${id}/password-reset`, {
      method: 'POST',
    })
  }

  async function unlock(id: string): Promise<void> {
    await api(`/app-users/${id}/unlock`, { method: 'POST' })
  }

  return { list, create, update, disable, enable, resetPassword, unlock }
}
