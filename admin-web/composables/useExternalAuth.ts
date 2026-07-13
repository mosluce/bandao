import type {
  ConfigureExternalAuthRequest,
  OrgDto,
  SyncExternalUsersResponse,
  TestLoginRequest,
  TestLoginResponse,
} from '~/types/api'

/**
 * Wraps the external-database auth admin endpoints:
 * `PUT /orgs/me/external-auth` (set auth source + config),
 * `POST /orgs/me/external-auth/test-login` (dry-run against the external DB), and
 * `POST /orgs/me/external-auth/sync` (bulk-upsert the external user roster).
 */
export function useExternalAuth() {
  const api = useApi()

  async function configure(req: ConfigureExternalAuthRequest): Promise<OrgDto> {
    return api<OrgDto>('/orgs/me/external-auth', {
      method: 'POST',
      body: req,
    })
  }

  async function testLogin(req: TestLoginRequest): Promise<TestLoginResponse> {
    return api<TestLoginResponse>('/orgs/me/external-auth/test-login', {
      method: 'POST',
      body: req,
    })
  }

  async function sync(): Promise<SyncExternalUsersResponse> {
    return api<SyncExternalUsersResponse>('/orgs/me/external-auth/sync', {
      method: 'POST',
    })
  }

  return { configure, testLogin, sync }
}
