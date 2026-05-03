// Mirrors Rust DTOs in api/src/handlers/. OpenAPI codegen → ROADMAP.

export type Role = 'admin' | 'member'

export interface UserDto {
  id: string
  email: string
  role: Role
}

export interface OrgDto {
  id: string
  name: string
  code: string
  owner_id: string
  slug?: string
  slug_changed_at?: string
}

export type RemovalKind = 'kicked' | 'left'

export interface CooldownDto {
  email: string
  removed_at: string | null
  cooldown_until: string | null
  removal_kind: RemovalKind
}

export interface AuthResponse {
  user: UserDto
  org: OrgDto
  role: Role
}

export interface RotateCodeResponse {
  code: string
}

export interface SetSlugRequest {
  slug: string
}

export interface SetSlugResponse {
  slug: string
}

export interface DashboardUserDto {
  id: string
  email: string
  role: Role
}

export type RegisterRequest =
  | { mode: 'create'; email: string; password: string; org_name: string }
  | { mode: 'join'; email: string; password: string; org_code: string }

export interface LoginRequest {
  email: string
  password: string
}

export interface UpdateRoleRequest {
  role: Role
}

export interface ApiErrorBody {
  error: {
    code: string
    message: string
    retry_after?: string
  }
}

export class ApiError extends Error {
  readonly status: number
  readonly code: string
  readonly retryAfter: string | null

  constructor(status: number, code: string, message: string, retryAfter: string | null = null) {
    super(message)
    this.status = status
    this.code = code
    this.retryAfter = retryAfter
    this.name = 'ApiError'
  }
}
