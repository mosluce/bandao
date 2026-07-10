## 1. Data model & validation (api)

- [x] 1.1 Add an `EncryptMode` enum (`Off | Optional | Required`, serde rename to `off`/`optional`/`required`) and add `encrypt: EncryptMode` + `trust_server_certificate: bool` to `domain::ExternalAuthConfig`, both with `#[serde(default = ...)]` (`Optional` / `true`) so existing documents deserialize unchanged
- [x] 1.2 `encrypt` validity is enforced at the type/deserialization layer: `ExternalAuthInput.encrypt: EncryptMode` (serde rename off/optional/required) rejects any other value with a 400 before persistence — no change to `validate_query_settings` needed; existing placeholder/key_col/display_col checks retained

## 2. MSSQL provider (api)

- [x] 2.1 In `auth/providers/mssql.rs`, map `encrypt` → `tiberius::EncryptionLevel` (`Off`/`On`/`Required`) via `cfg.encryption(...)`; replace the unconditional `cfg.trust_cert()` with `if self.config.trust_server_certificate { cfg.trust_cert() }`
- [x] 2.2 Confirm the diagnostic on handshake failure still surfaces as `Unavailable` (so test-login shows a useful message when the wrong encrypt mode is chosen)

## 3. API surface (api)

- [x] 3.1 Add `encrypt` + `trust_server_certificate` to `ExternalAuthSummaryDto` (non-secret — returned as-is, unlike the password) and to the configure endpoint input; persist both on save
- [x] 3.2 Ensure the test-login dry-run picks up both fields (it reuses the same config — verify no separate path drops them)

## 4. admin-web

- [x] 4.1 Extend `types/api` `ExternalAuthInput` + the external_auth summary type with `encrypt` (union `'off' | 'optional' | 'required'`) and `trust_server_certificate: boolean`
- [x] 4.2 Add to `pages/settings/auth.vue`: an Encrypt `<select>` (off / optional / required, default optional) and a Trust server certificate checkbox (default checked); seed them from the loaded summary; include them in save + test-login payloads

## 5. Docs, spec & verification

- [x] 5.1 Update the `external-db-auth` spec delta: connection-config requirement gains `encrypt` + `trust_server_certificate` (with defaults, non-secret surfacing); validation requirement rejects an invalid `encrypt`
- [x] 5.2 `cargo test` + `cargo clippy` clean; admin-web `nuxt typecheck` + build clean
- [ ] 5.3 **PENDING (needs live customer server + creds)**: verify against real KLCC (`erp.klcc.com.tw`) via admin-web test-login — try `optional`/`off` where `required` failed, confirm identity columns resolve, record the working mode. Code path is covered by the existing dockerized external_auth integration tests + unit mapping; only the real-server encrypt-mode combo remains to confirm.
