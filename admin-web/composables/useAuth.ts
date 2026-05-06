import type {
  AuthResponse,
  CreateOrgRequest,
  JoinOrgRequest,
  LoginRequest,
  MembershipDto,
  OrgDto,
  RegisterRequest,
  Role,
  SwitchOrgRequest,
  TransferOwnerRequest,
  UserDto,
} from '~/types/api'
import { ApiError } from '~/types/api'

interface AuthState {
  loaded: boolean
  data: AuthResponse | null
}

const LAST_ORG_KEY = 'bandao.lastSelectedOrgId'

function readLastOrg(): string | null {
  if (typeof localStorage === 'undefined') return null
  return localStorage.getItem(LAST_ORG_KEY)
}

function writeLastOrg(orgId: string | null) {
  if (typeof localStorage === 'undefined') return
  if (orgId) localStorage.setItem(LAST_ORG_KEY, orgId)
  else localStorage.removeItem(LAST_ORG_KEY)
}

export function useAuth() {
  const api = useApi()
  const state = useState<AuthState>('bandao.auth', () => ({
    loaded: false,
    data: null,
  }))

  const me = computed(() => state.value.data)
  const user = computed<UserDto | null>(() => state.value.data?.user ?? null)
  const memberships = computed<MembershipDto[]>(() => state.value.data?.memberships ?? [])
  const currentOrg = computed<OrgDto | null>(() => state.value.data?.current_org ?? null)
  const role = computed<Role | null>(() => state.value.data?.role ?? null)
  const isAuthenticated = computed(() => state.value.data !== null)
  const isAdmin = computed(() => role.value === 'admin')
  const isOwner = computed(() => {
    const u = user.value
    const o = currentOrg.value
    return !!u && !!o && u.id === o.owner_id
  })

  /** Persist server-confirmed current org to localStorage for next visit. */
  function persistCurrent(data: AuthResponse | null) {
    writeLastOrg(data?.current_org?.id ?? null)
  }

  /**
   * After loading state from the server, prefer the user's last selection if
   * it's still a valid membership and differs from server's pick. Calls
   * /me/current-org to align server-side and updates local state.
   */
  async function alignWithLastSelection(data: AuthResponse): Promise<AuthResponse> {
    const desired = readLastOrg()
    if (!desired) {
      persistCurrent(data)
      return data
    }
    const owns = data.memberships.some(m => m.org.id === desired)
    if (!owns) {
      // Stored selection no longer valid (left that org); fall back to server's pick.
      persistCurrent(data)
      return data
    }
    if (data.current_org?.id === desired) {
      // Already aligned — nothing to do.
      return data
    }
    // Sync server to the local preference.
    const switched = await api<AuthResponse>('/me/current-org', {
      method: 'POST',
      body: { org_id: desired } satisfies SwitchOrgRequest,
    })
    state.value = { loaded: true, data: switched }
    persistCurrent(switched)
    return switched
  }

  async function refresh() {
    try {
      const data = await api<AuthResponse>('/me', { method: 'GET' })
      state.value = { loaded: true, data }
      await alignWithLastSelection(data)
    }
    catch (err) {
      if (err instanceof ApiError && err.status === 401) {
        state.value = { loaded: true, data: null }
        writeLastOrg(null)
        return
      }
      throw err
    }
  }

  async function ensureLoaded() {
    if (!state.value.loaded) await refresh()
  }

  async function login(req: LoginRequest) {
    const data = await api<AuthResponse>('/auth/login', {
      method: 'POST',
      body: req,
    })
    state.value = { loaded: true, data }
    await alignWithLastSelection(data)
  }

  async function register(req: RegisterRequest) {
    const data = await api<AuthResponse>('/auth/register', {
      method: 'POST',
      body: req,
    })
    state.value = { loaded: true, data }
    persistCurrent(data)
  }

  async function logout() {
    try {
      await api('/auth/logout', { method: 'POST' })
    }
    finally {
      state.value = { loaded: true, data: null }
      writeLastOrg(null)
    }
  }

  /** Logged-in user creates a brand-new Org and becomes its owner. */
  async function createOrg(orgName: string) {
    const data = await api<AuthResponse>('/me/orgs', {
      method: 'POST',
      body: { org_name: orgName } satisfies CreateOrgRequest,
    })
    state.value = { loaded: true, data }
    persistCurrent(data)
  }

  /** Logged-in user joins an existing Org via org_code or slug. */
  async function joinOrg(orgCode: string) {
    const data = await api<AuthResponse>('/me/memberships', {
      method: 'POST',
      body: { org_code: orgCode } satisfies JoinOrgRequest,
    })
    state.value = { loaded: true, data }
    persistCurrent(data)
  }

  /** Switch the current session's active Org. Target must be in memberships. */
  async function switchOrg(orgId: string) {
    const data = await api<AuthResponse>('/me/current-org', {
      method: 'POST',
      body: { org_id: orgId } satisfies SwitchOrgRequest,
    })
    state.value = { loaded: true, data }
    persistCurrent(data)
  }

  /**
   * Leave the current org. Server force-kicks sessions for that org including
   * this one, so we drop local auth state afterwards.
   */
  async function leaveOrg() {
    await api('/me/leave', { method: 'POST' })
    state.value = { loaded: true, data: null }
    writeLastOrg(null)
  }

  /**
   * Transfer ownership of `currentOrg` to another admin. Caller must currently
   * be the owner; `currentPassword` is re-verified server-side.
   */
  async function transferOwnership(newOwnerUserId: string, currentPassword: string) {
    await api('/orgs/me/owner', {
      method: 'POST',
      body: {
        new_owner_user_id: newOwnerUserId,
        current_password: currentPassword,
      } satisfies TransferOwnerRequest,
    })
    // owner_id changed for the org — refresh to pick up the new shape.
    await refresh()
  }

  return {
    me,
    user,
    memberships,
    currentOrg,
    role,
    isAuthenticated,
    isAdmin,
    isOwner,
    refresh,
    ensureLoaded,
    login,
    register,
    logout,
    createOrg,
    joinOrg,
    switchOrg,
    leaveOrg,
    transferOwnership,
  }
}
