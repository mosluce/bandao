// Mirrors Rust DTOs in api/src/handlers/. OpenAPI codegen → ROADMAP.

export type Role = 'admin' | 'member'

export interface UserDto {
  id: string
  email: string
}

export interface OrgDto {
  id: string
  name: string
  code: string
  owner_id: string
  timezone: string
  checkin: { transfer_enabled: boolean }
  slug?: string
  slug_changed_at?: string
}

export interface MembershipDto {
  org: OrgDto
  role: Role
}

export interface AuthResponse {
  user: UserDto
  memberships: MembershipDto[]
  /** `null` when the user has no memberships or no current org selected. */
  current_org: OrgDto | null
  /** Role within `current_org`. `null` whenever `current_org` is `null`. */
  role: Role | null
}

export type RemovalKind = 'kicked' | 'left'

export interface CooldownDto {
  email: string
  removed_at: string | null
  cooldown_until: string | null
  removal_kind: RemovalKind
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

export interface CreateOrgRequest {
  org_name: string
}

export interface JoinOrgRequest {
  org_code: string
}

export interface SwitchOrgRequest {
  org_id: string
}

export interface TransferOwnerRequest {
  new_owner_user_id: string
  current_password: string
}

export interface UpdateRoleRequest {
  role: Role
}

// --- AppUser ---

export type AppUserStatus = 'active' | 'disabled'

export interface AppUserDto {
  id: string
  username: string
  display_name: string
  status: AppUserStatus
  needs_password_change: boolean
  last_login_at?: string
  created_at: string
}

export interface CreateAppUserRequest {
  username: string
  display_name: string
}

export interface UpdateAppUserRequest {
  display_name?: string
  status?: AppUserStatus
}

export interface CreateAppUserResponse {
  user: AppUserDto
  initial_password: string
}

export type PasswordResetResponse = CreateAppUserResponse

// --- Checkin events ---

export type CheckinEventType = 'clock_in' | 'clock_out' | 'transfer_out' | 'transfer_in'
export type AppUserCheckinStatus = 'off_duty' | 'on_site' | 'in_transit'
export type EventSource = 'app' | 'admin_force'
export type EventInitiatorKind = 'app_user' | 'dashboard_user'

export interface GeoPoint {
  lat: number
  lng: number
}

export interface EventLocation {
  coordinates: GeoPoint
  accuracy_meters?: number
  region_name?: string
  manual_label?: string
}

export interface CheckinEventDto {
  id: string
  app_user_id: string
  event_type: CheckinEventType
  occurred_at_client: string
  occurred_at_server: string
  source: EventSource
  initiated_by_kind: EventInitiatorKind
  initiated_by_id: string
  location: EventLocation
  reason?: string
  has_skew_warning: boolean
}

export interface BoardAppUserDto {
  id: string
  username: string
  display_name: string
}

export interface CheckinUserBoardRowDto {
  user: BoardAppUserDto
  status: AppUserCheckinStatus
  current_shift_started_at?: string
  last_event?: CheckinEventDto
  has_skew_warning: boolean
}

export interface ForceCheckoutRequest {
  reason?: string
}

export interface UpdateOrgSettingsRequest {
  transfer_enabled?: boolean
  timezone?: string
}

export interface OrgCheckinSettings {
  transfer_enabled: boolean
}

export interface OrgSettingsDto {
  timezone: string
  checkin: OrgCheckinSettings
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
