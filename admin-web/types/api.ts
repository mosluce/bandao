// Mirrors Rust DTOs in api/src/handlers/. OpenAPI codegen → ROADMAP.

export type Role = 'admin' | 'member'

export interface UserDto {
  id: string
  email: string
}

export type OrgAuthSource = 'internal' | 'external_db'

/** Password-free view of an Org's external-auth config (mirrors the server;
 * the connection password is never sent to the client). */
export type EncryptMode = 'off' | 'optional' | 'required'

export interface ExternalAuthSummaryDto {
  driver: string
  host: string
  port: number
  database: string
  username: string
  query: string
  key_col: string
  display_col: string
  password_set: boolean
  encrypt: EncryptMode
  trust_server_certificate: boolean
}

export interface OrgDto {
  id: string
  name: string
  code: string
  owner_id: string
  timezone: string
  checkin: { transfer_enabled: boolean, location_tracking_enabled: boolean }
  auth_source: OrgAuthSource
  external_auth?: ExternalAuthSummaryDto
  slug?: string
  slug_changed_at?: string
}

/** Connection + query settings as submitted by an admin. `password` is
 * write-only: omit it to keep the stored one. */
export interface ExternalAuthInput {
  driver: string
  host: string
  port: number
  database: string
  username: string
  password?: string
  query: string
  key_col: string
  display_col: string
  encrypt: EncryptMode
  trust_server_certificate: boolean
}

export interface ConfigureExternalAuthRequest {
  auth_source: OrgAuthSource
  external_auth?: ExternalAuthInput
}

export interface TestLoginRequest {
  external_auth: ExternalAuthInput
  test_account: string
  test_password: string
}

export interface TestLoginResponse {
  connected: boolean
  matched: boolean
  external_key?: string
  display_name?: string
  error?: string
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

export type AppUserAuthSource = 'internal' | 'external'

export interface AppUserDto {
  id: string
  auth_source: AppUserAuthSource
  /** Present for internal users. */
  username?: string
  /** Present for external shadow users. */
  external_key?: string
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
  location_tracking_enabled?: boolean
  timezone?: string
}

export interface OrgCheckinSettings {
  transfer_enabled: boolean
  location_tracking_enabled: boolean
}

export interface OrgSettingsDto {
  timezone: string
  checkin: OrgCheckinSettings
}

export interface LocationPingDto {
  id: string
  app_user_id: string
  lat: number
  lng: number
  accuracy_meters?: number
  occurred_at_client: string
  occurred_at_server: string
}

export interface LocationListParams {
  before?: string
  limit?: number
  from?: string
  to?: string
}

export type JoinRequestStatus = 'pending' | 'approved' | 'rejected' | 'cancelled'

export interface JoinRequestDto {
  id: string
  org: { id: string, name: string, code: string }
  status: JoinRequestStatus
  application_message?: string
  rejection_reason?: string
  requested_at: string
  decided_at?: string
}

export interface OrgPendingJoinRequestDto {
  id: string
  user_id: string
  email: string
  status: JoinRequestStatus
  application_message?: string
  rejection_reason?: string
  requested_at: string
  decided_at?: string
}

// --- Org API tokens ---

/** Known, closed set of capabilities an API token can be scoped to — mirrors
 * the server's `ApiTokenScope` enum. Not free text: the create-token form
 * renders this list as checkboxes. */
export type ApiTokenScope = 'checkin:read'

export const API_TOKEN_SCOPES: { value: ApiTokenScope, label: string }[] = [
  { value: 'checkin:read', label: '打卡紀錄唯讀匯出（震旦雲等外部系統排程呼叫用）' },
]

export type ApiTokenStatus = 'active' | 'disabled'

export interface ApiTokenDto {
  id: string
  name: string
  scopes: ApiTokenScope[]
  status: ApiTokenStatus
  token_prefix: string
  created_at: string
  last_used_at?: string
  rotated_at?: string
}

export interface CreateApiTokenRequest {
  name: string
  scopes: ApiTokenScope[]
}

export interface UpdateApiTokenStatusRequest {
  status: ApiTokenStatus
}

/** Creation/rotation response — `secret` is the plaintext token, present
 * exactly once. No other endpoint ever returns it again. */
export interface ApiTokenSecretResponse {
  token: ApiTokenDto
  secret: string
}

export interface SubmitJoinRequestRequest {
  org_code: string
  application_message?: string
}

export interface RejectJoinRequestRequest {
  rejection_reason?: string
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
