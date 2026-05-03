import type {
  AuthResponse,
  LoginRequest,
  RegisterRequest,
  Role,
} from '~/types/api'
import { ApiError } from '~/types/api'

interface AuthState {
  loaded: boolean
  data: AuthResponse | null
}

export function useAuth() {
  const api = useApi()
  const state = useState<AuthState>('argus.auth', () => ({
    loaded: false,
    data: null,
  }))

  const me = computed(() => state.value.data)
  const isAuthenticated = computed(() => state.value.data !== null)
  const role = computed<Role | null>(() => state.value.data?.role ?? null)
  const isAdmin = computed(() => role.value === 'admin')

  async function refresh() {
    try {
      const data = await api<AuthResponse>('/me', { method: 'GET' })
      state.value = { loaded: true, data }
    }
    catch (err) {
      if (err instanceof ApiError && err.status === 401) {
        state.value = { loaded: true, data: null }
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
  }

  async function register(req: RegisterRequest) {
    const data = await api<AuthResponse>('/auth/register', {
      method: 'POST',
      body: req,
    })
    state.value = { loaded: true, data }
  }

  async function logout() {
    try {
      await api('/auth/logout', { method: 'POST' })
    }
    finally {
      state.value = { loaded: true, data: null }
    }
  }

  return {
    me,
    role,
    isAdmin,
    isAuthenticated,
    refresh,
    ensureLoaded,
    login,
    register,
    logout,
  }
}
