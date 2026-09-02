# Security Phase 2: Direct OpenID Connect Identity Federation

**Status:** Deferred specification; not implemented  
**Audience:** PixieVault maintainers, contributors, and security reviewers  
**Scope:** Native host authentication only; guest applications remain unchanged  
**Recommended default:** Local passphrase and platform-backed user verification, with optional direct OpenID Connect (OIDC)

## 1. Decision Summary

PixieVault will support optional identity federation through direct OpenID Connect. A user may authenticate with a compatible identity provider, such as Google, Microsoft Entra ID, Keycloak, Authentik, or Okta, without PixieVault requiring a common identity broker or a paid hosted subscription.

Local authentication remains the out-of-box experience. A fresh installation must work without an internet connection, cloud account, identity provider, or PixieVault-operated service. Federated sign-in requires the person distributing or administering the build to register PixieVault as a public native client with each provider and supply a client ID. A client secret must never be compiled into or distributed with the desktop application.

Direct OIDC is an authentication and authorization gate. It does not replace Phase 0 encryption, become a vault encryption key, or allow a provider token to decrypt data. After federated authentication succeeds, PixieVault must still release the Vault Master Key through an approved local mechanism such as Windows Hello or the vault passphrase.

This design deliberately defers RADIUS, LDAP binds, SAML, a PixieVault-hosted broker, and provider-specific social-login adapters. Enterprises can initially connect any standards-compliant OIDC service or federation broker through the generic provider interface. Additional enterprise behavior can be added when a concrete requirement exists.

## 2. Goals

Phase 2 must:

- Preserve a fully local mode with no account, subscription, or network dependency.
- Add optional direct OIDC authentication using the system browser.
- Support standards-compliant providers through issuer discovery and a public client ID.
- Offer tested configuration presets for Google and Microsoft while retaining a generic OIDC option.
- Keep authentication, token handling, policy evaluation, and key release in the trusted native host.
- Keep guest applications and their source code unchanged.
- Separate external identity proof from authorization and cryptographic key release.
- Prevent a federated account from silently becoming the only way to recover a vault.
- Provide an extension point for a future enterprise broker or provider-specific adapter without replacing the core design.
- Minimize data collection, scopes, retained tokens, and external dependencies.

## 3. Non-Goals

Phase 2 will not:

- Implement RADIUS, direct LDAP, Active Directory password authentication, or Integrated Windows Authentication.
- Implement SAML directly. An organization may place a SAML-to-OIDC broker or identity platform in front of PixieVault.
- Operate a PixieVault identity service, account directory, key escrow service, or synchronization service.
- Store or process a user's Google, Microsoft, or enterprise password.
- Use an embedded WebView for provider authentication.
- Support the OAuth implicit grant, Resource Owner Password Credentials grant, or a desktop-embedded client secret.
- Treat email address, display name, or tenant domain as a globally stable user identifier.
- Give guest applications access to identity-provider tokens or security-sensitive host commands.
- Make network identity the source of vault encryption keys.
- Add provider login code to guest applications.
- Promise offline federated revalidation in the initial release.

Facebook Login and cross-platform Sign in with Apple are deferred. They require provider-specific behavior that does not fit the initial generic public-client contract. Apple's web token exchange requires a signed client secret backed by a developer private key; such a key must not be shipped in a desktop client. Native Apple-platform integration or a trusted backend can be considered separately.

## 4. Meaning of “Out of the Box”

PixieVault must be useful immediately after installation:

1. The user creates or opens a vault with the existing local authentication and Phase 0 encryption flow.
2. Platform-backed user verification, such as Windows Hello, may be enrolled after the vault is secured.
3. Federated authentication remains disabled until an owner or administrator configures a provider.

Third-party sign-in cannot be universally enabled by source code alone. OAuth and OIDC providers require an application registration, an approved redirect URI, and a client ID. Provider policies, consent screens, quotas, and verification obligations belong to the registration owner.

The community distribution model is therefore **bring your own public-client registration**. Setup documentation and presets should make registration straightforward, but the repository must not contain shared secrets. A future official distribution may use maintainer-owned public client IDs only after governance, abuse handling, privacy, provider-policy, quota, and continuity responsibilities are accepted and documented.

## 5. Supported Authentication Modes

### 5.1 Local-Only Mode

- Default for new and existing vaults.
- Requires no provider configuration or network access.
- Uses the vault passphrase and optional platform-backed key release.
- Remains an available recovery path unless a future managed-enterprise policy explicitly changes that contract.

### 5.2 Direct OIDC Mode

- Optional additional authentication gate.
- Configured with a trusted issuer URL, public client ID, redirect behavior, and authorization policy.
- Uses Authorization Code flow with PKCE and the operating system's browser.
- Normalizes a successful identity to the immutable pair `(issuer, subject)`.
- Requires local key release before encrypted vault data becomes available.

### 5.3 Future Brokered or Enterprise Mode

The generic OIDC contract must accept an enterprise identity platform or common federation broker as its issuer. Entra ID, Okta, Keycloak, Authentik, and similar products can federate upstream directories or social providers without PixieVault understanding the upstream protocol.

Provider-specific features such as complex group-to-role mappings, device compliance, Continuous Access Evaluation, SAML ingestion, SCIM provisioning, and organization-wide policy enforcement are future work.

## 6. Initial Provider Policy

| Provider type | Phase 2 position | Notes |
| --- | --- | --- |
| Generic standards-compliant OIDC | Required | Configure issuer and public client ID; validate discovery metadata before saving. |
| Google | Supported preset | Uses Google's OIDC endpoints and a separately registered desktop/public client. |
| Microsoft identity platform / Entra ID | Supported preset | Tenant policy must be explicit: single tenant, organizations, consumers, or combined audience. |
| Keycloak, Authentik, Okta, and similar | Supported through generic OIDC | Provider must support the required native public-client flow, PKCE S256, and redirect method. |
| Common identity federation broker | Compatible but not required | Configure it as another OIDC issuer; PixieVault has no broker subscription dependency. |
| Facebook Login | Deferred | Add only through a reviewed provider adapter if its current protocol and policy requirements justify it. |
| Sign in with Apple | Deferred | Cross-platform token exchange requires secret-key handling unsuitable for an untrusted public desktop client. |
| SAML, LDAP, RADIUS | Deferred | Use an OIDC-capable identity platform or broker when needed. |

“Generic OIDC” is not a claim that every provider behaves identically. Setup must probe discovery metadata and reject providers that cannot satisfy PixieVault's security requirements. Provider interoperability must be tested, not inferred.

## 7. Security Architecture

The authentication sequence is:

```text
PixieVault host
    -> starts one bounded login transaction
    -> opens the system browser
    -> identity provider authenticates the user
    -> browser returns an authorization code to an ephemeral loopback listener
    -> host validates transaction state and exchanges the code with PKCE
    -> host validates the ID token and normalizes (issuer, subject)
    -> host evaluates the vault authorization policy
    -> Windows Hello or the local passphrase releases the Vault Master Key
    -> host decrypts and materializes the workspace
```

The trust boundaries are strict:

- The provider proves an external identity.
- PixieVault policy decides whether that identity may access a specific vault.
- Windows Hello or the vault passphrase releases locally protected key material.
- Phase 0 cryptography protects data at rest.
- Guest applications receive only the minimum session information required by the existing host contract; they never receive provider tokens, vault keys, or identity configuration.

Federated success alone must never transition the vault to an unlocked state.

## 8. Native Application Protocol Requirements

The implementation must follow OAuth 2.0 for Native Apps, OIDC Core, OIDC Discovery, PKCE, and the current OAuth Security Best Current Practice.

### 8.1 Authorization Flow

- Use Authorization Code flow with PKCE.
- Require the `S256` PKCE challenge method. Do not fall back to `plain`.
- Launch the operating system's external browser. Do not use the application WebView, an iframe, or an embedded credential form.
- Treat PixieVault as a public native client that cannot keep a shared client secret.
- Request `response_type=code` and the minimum scopes `openid profile email` unless fewer claims satisfy the configured policy.
- Generate a cryptographically random, transaction-specific PKCE verifier, `state`, and OIDC `nonce`.
- Bind all transaction values to one login attempt and accept them only once.
- Set a short, bounded transaction timeout and support explicit cancellation.
- Permit only one active login transaction per host window or vault.

### 8.2 Loopback Redirect

- Bind an ephemeral listener to the IPv4 literal `127.0.0.1` on an operating-system-assigned port.
- Never bind the callback listener to `0.0.0.0`, a LAN address, or a public interface.
- Use a random, transaction-specific callback path.
- Register the complete redirect URI as required by the provider. Dynamic loopback ports may be used only when the provider supports the native-app exception.
- Accept only the expected path, HTTP method, `state`, and one callback request.
- Return a small local completion page with no third-party resources and `Referrer-Policy: no-referrer`.
- Do not place tokens in the callback response. The browser returns only an authorization code and protocol state.
- Close the listener immediately after success, failure, cancellation, timeout, vault lock, or application exit.

If a provider does not accept the required loopback model, it is unsupported until a secure, platform-appropriate redirect adapter is designed and reviewed.

### 8.3 Discovery and Endpoint Validation

- Accept only an administrator-configured HTTPS issuer, except for a deliberately isolated local test issuer in automated tests.
- Fetch `/.well-known/openid-configuration` according to OIDC Discovery.
- Require exact equality between the configured issuer, discovered `issuer`, and ID token `iss` claim.
- Require HTTPS authorization, token, and JWKS endpoints in production.
- Do not allow discovery to turn provider configuration into an unrestricted server-side request primitive. Reject loopback, private, link-local, multicast, unspecified, and otherwise unsafe destination addresses unless the user is in an explicit local-development mode.
- Apply bounded timeouts, response-size limits, redirect limits, and content-type validation to discovery and JWKS requests.
- Pin the login transaction to its discovered endpoints so an authorization response cannot select a different issuer or token endpoint.
- Cache discovery documents and JWKS only with bounded lifetimes and safe key-rotation behavior.

### 8.4 ID Token Validation

Before accepting an identity, the host must validate at least:

- A permitted signing algorithm; reject `none` and algorithm confusion.
- The signature against the issuer's validated JWKS.
- Exact `iss` match.
- `aud` contains the configured client ID.
- `azp` when required for a token with multiple audiences.
- `exp`, `iat`, and `nbf` when present, using a small documented clock-skew allowance.
- Exact transaction `nonce` match.
- Required authentication context or assurance claims when policy asks for them.
- Provider-specific claims only through a reviewed adapter or preset.

An unknown signing key ID may trigger one bounded JWKS refresh. Validation failure must fail closed and must not unlock, link an identity, or mutate protected vault state.

## 9. Identity and Authorization Model

The canonical external identity key is:

```text
(normalized exact issuer URL, provider subject claim)
```

Email, username, display name, tenant display name, and domain are mutable attributes. They may be displayed or used as secondary policy inputs, but they must not replace `(iss, sub)` for account identity or linking.

Recommended initial authorization policy:

- Federated authentication is optional and off by default.
- A secured existing vault can link an external identity only after fresh local authentication.
- The initial implementation supports one owner identity per vault; the schema may permit multiple identities later without changing the identity key.
- A new federated identity can never claim an existing vault merely because its email resembles a local user or previous identity.
- Relinking, unlinking, provider changes, and authorization-policy changes require an unlocked vault plus fresh local verification.
- Removing the final local recovery method is prohibited in Phase 2.
- Domain allowlists alone are insufficient authorization. If offered for convenience, they must be paired with an explicit issuer/tenant restriction and a clear warning.
- Roles or group claims are denied by default unless their issuer, claim name, expected format, and mapping are explicitly configured.

For Microsoft, tenant selection and accepted issuer patterns require special care. A multi-tenant endpoint must not imply that every tenant is authorized. The normalized token issuer and tenant claims must satisfy the configured policy.

## 10. Relationship to Encryption and Windows Hello

OIDC answers “which external identity just authenticated?” It does not answer “what is the vault decryption key?”

The Vault Master Key remains random local key material protected by the Phase 0 design. A successful federated sign-in can satisfy an authorization gate but cannot derive, wrap, replace, transmit, or escrow the Vault Master Key. The key is released only after local user verification:

- Windows Hello or another approved platform-backed mechanism unwraps locally sealed key material; or
- the user enters the vault passphrase and the host completes the existing key-derivation flow.

This split prevents loss of a provider account, network outage, provider compromise, or provider migration from silently changing the cryptographic protection of vault data. It also avoids making an ID token, access token, refresh token, email address, or provider password an encryption secret.

On vault lock, application exit, session timeout, or identity-policy failure, the host must clear provider tokens, normalized session identity, unwrapped key material, and materialized workspace data according to the Phase 0 lock contract.

## 11. Configuration and Storage

Provider configuration is host security state and must be stored inside the encrypted vault or another host-owned encrypted security store. It must not be placed in guest app directories, manifests, browser storage, command-line arguments, logs, or plaintext configuration files.

Illustrative schema:

```json
{
  "version": 1,
  "providers": [
    {
      "id": "google-primary",
      "kind": "oidc",
      "display_name": "Google",
      "issuer": "https://accounts.google.com",
      "client_id": "PUBLIC_NATIVE_CLIENT_ID",
      "scopes": ["openid", "profile", "email"],
      "allowed_identities": [
        {
          "issuer": "https://accounts.google.com",
          "subject": "PROVIDER_SUBJECT"
        }
      ],
      "authorization": {
        "default_role": "vault-owner"
      },
      "offline_access": {
        "enabled": false,
        "max_age_hours": 0
      }
    }
  ]
}
```

The final schema must be versioned, validated before persistence, and migrated transactionally. Unknown security-critical fields must not be silently accepted. Existing vaults migrate to an empty provider list and retain local-only behavior.

Client IDs are identifiers, not secrets. Nevertheless, configuration changes remain privileged because a malicious issuer or client substitution could redirect authentication to an attacker's registration.

## 12. Token Handling

- Keep ID and access tokens in host memory only for the shortest practical period.
- Do not expose raw tokens through Tauri commands, events, the JavaScript bridge, guest applications, crash reports, analytics, or diagnostics.
- Redact authorization codes, PKCE values, cookies, tokens, subjects, emails, and provider error details from logs by default.
- Do not request a refresh token or `offline_access` in the initial implementation.
- If refresh tokens become necessary, add a separate threat model and store them only under vault encryption or an approved platform credential mechanism. Implement rotation, revocation, expiry, and sign-out behavior before enabling them.
- Clear sensitive buffers where practical and avoid unnecessary string copies.
- Revalidate token expiry and the active authorization policy before every transition into an authenticated host session.
- Provider logout is not assumed to terminate all upstream browser sessions. PixieVault sign-out must always terminate the local session and lock the vault.

## 13. Host API Boundary

OIDC commands belong to the trusted host shell, not the guest bridge. A possible internal command surface is:

```text
pv_oidc_list_providers
pv_oidc_validate_provider_configuration
pv_oidc_configure_provider
pv_oidc_remove_provider
pv_oidc_begin_login
pv_oidc_cancel_login
pv_oidc_get_session_summary
pv_oidc_sign_out
```

Names are illustrative. The implementation must enforce command origin and authorization in Rust rather than relying on hidden UI controls. Provider configuration commands require an unlocked vault and fresh local verification. Session summaries must expose only display-safe information and must never contain tokens.

Guest apps must not be able to start provider login, configure an issuer, link an identity, inspect claims, or sign out the host unless a future capability is explicitly designed, permissioned, and reviewed.

## 14. Suggested Native Module Boundaries

The exact library selection is an implementation decision, but responsibilities should remain independently testable:

```text
src-tauri/src/auth/oidc/
├── mod.rs          # Public host-owned orchestration
├── config.rs       # Versioned configuration and validation
├── discovery.rs    # Issuer metadata and JWKS retrieval
├── pkce.rs         # Transaction secrets and challenges
├── callback.rs     # Bounded loopback receiver
├── tokens.rs       # Code exchange and ID token validation
├── identity.rs     # (issuer, subject) normalization
└── policy.rs       # Vault authorization decisions
```

Prefer a small `IdentityProvider` abstraction only after the generic OIDC path is working. Do not create an abstraction that weakens validation to accommodate non-conforming providers.

## 15. User Experience

The host should show:

- **Unlock locally** as the reliable default action.
- **Sign in with _provider_** only for configured and validated providers.
- A provider setup flow that explains the issuer, public client ID, redirect URI, scopes, and local recovery requirement.
- A clear transition from “identity verified” to the separate Windows Hello or passphrase key-release step.
- Provider-specific errors without displaying raw responses, tokens, claims, or internal endpoints.
- Explicit offline behavior: local unlock remains available; online federated authentication is unavailable.

Errors should be typed and actionable, including not configured, invalid issuer, provider incompatible, browser launch failed, callback unavailable, timeout, cancelled, state mismatch, token exchange rejected, token validation failed, identity not authorized, and network unavailable.

Authentication failure must not create a vault, modify identity links, weaken local security, or leave a callback listener running.

## 16. Privacy Requirements

- Request only claims needed for identification and policy.
- Explain why profile and email scopes are requested before redirecting.
- Do not send vault names, app inventory, vault data, device telemetry, or encryption metadata to an identity provider.
- Do not add authentication telemetry by default.
- Treat issuer, subject, tenant, email, and authentication timestamps as personal data.
- Document what is retained locally and how unlinking or deleting a vault removes it.
- Keep provider logos and UI assets packaged locally rather than loading them from third parties at runtime.

## 17. Threat Model and Security Limits

The design addresses:

- Password collection by PixieVault.
- Authorization-code interception through PKCE.
- Login CSRF and response injection through transaction-bound state and nonce.
- Provider mix-up through exact issuer and endpoint binding.
- Token forgery through signature and claim validation.
- Account confusion through `(iss, sub)` identity keys.
- Malicious redirect or callback reuse through exact, one-shot loopback handling.
- Guest application attempts to obtain provider tokens or modify authentication policy.
- Offline disk theft through the existing Phase 0 encryption and lock design.

The design does not protect against:

- A compromised operating system, kernel, administrator account, or PixieVault process while the vault is unlocked.
- Compromise of the selected identity provider or the user's provider account.
- A malicious build or dependency with access to the trusted host process.
- Data deliberately exported by the user or exposed by an authorized guest application while unlocked.
- Recovery failure if the user loses every configured local recovery factor.

These limits must be stated in user and administrator documentation.

## 18. Testing Requirements

Automated tests must use a local deterministic mock OIDC issuer. CI must not depend on real Google, Microsoft, or enterprise accounts.

### 18.1 Unit and Component Tests

- Discovery rejects issuer mismatch, insecure endpoints, unsafe network targets, oversized responses, and redirect abuse.
- PKCE verifier generation and `S256` challenge computation match the standard.
- `state` and `nonce` are random, transaction-bound, single-use, and rejected on mismatch.
- The callback binds only to loopback, accepts only the expected request, and closes on every terminal path.
- ID token validation covers signature, permitted algorithms, `iss`, `aud`, `azp`, `exp`, `iat`, `nbf`, and `nonce`.
- JWKS key rotation succeeds through one bounded refresh; unknown or ambiguous keys fail closed.
- Identity linking uses `(iss, sub)` and cannot be taken over by an equal email address.
- Tenant and allowlist policies deny by default.
- Expiry, cancellation, sign-out, auto-lock, and process exit clear sensitive state.
- Logs and error objects contain no codes, PKCE values, tokens, subjects, or emails.
- Guest application origins cannot invoke privileged OIDC commands.
- Failed or interrupted authentication never unlocks or mutates the vault.
- Existing vault migration produces local-only configuration without changing guest application data.
- Offline behavior preserves local unlock and never treats stale network identity as freshly authenticated.

### 18.2 Integration and Acceptance Tests

- A fresh install creates and unlocks a local-only vault with no account or network.
- A standards-compliant mock provider can be configured using issuer and public client ID.
- The system browser opens, completes Authorization Code plus PKCE, and returns through loopback.
- Successful OIDC authentication does not expose vault data before local key release.
- Windows Hello and passphrase key release continue to work with OIDC enabled and disabled.
- Locking during browser authentication cancels the transaction and leaves the vault sealed.
- Provider configuration and linked identities persist only in encrypted host storage.
- Packaged Windows, Linux, and macOS builds exercise loopback behavior and system-browser return.
- Manual compatibility checks cover current Google and Microsoft registrations before release; untested providers are labeled unverified.
- No file beneath `apps/` changes as part of the implementation.

Security tests should include malformed JWTs, algorithm substitution, duplicate JSON claims, clock-boundary cases, Unicode issuer confusion, callback races, port squatting, replay, DNS rebinding/SSRF cases, malicious discovery metadata, and concurrent login attempts.

## 19. Delivery Plan

Implementation should be split into reviewable stages:

1. Record the architecture decision and freeze the provider/security contract.
2. Build the mock OIDC issuer and protocol conformance tests first.
3. Implement provider configuration validation, safe discovery, and JWKS handling.
4. Implement the one-shot loopback receiver and Authorization Code plus PKCE transaction.
5. Implement strict ID token validation and normalized external identity.
6. Implement vault authorization and identity linking behind fresh local verification.
7. Integrate the separate Windows Hello/passphrase key-release step.
8. Add trusted-host setup and sign-in UI with redacted diagnostics.
9. Add Google and Microsoft configuration presets without provider secrets.
10. Validate packaged builds on Windows, Linux, and macOS.
11. Complete a focused security review, dependency audit, and negative-test pass.
12. Release behind a disabled-by-default feature flag until all acceptance criteria pass.

No stage may modify guest application source. If a future guest-facing identity capability is needed, it requires a separate specification and migration contract.

## 20. Acceptance Criteria

Phase 2 is complete only when:

- PixieVault remains fully usable in local-only mode without network access or a subscription.
- A compatible provider can be configured with an issuer and public native client ID, without a client secret.
- Authentication uses the system browser, Authorization Code flow, and PKCE S256.
- Discovery, endpoints, redirect response, ID token, issuer, audience, nonce, time claims, and signature are validated fail-closed.
- External users are keyed by `(iss, sub)`, not email.
- Identity linking and provider configuration require fresh local verification.
- Federated authentication alone cannot release the Vault Master Key or decrypt data.
- Tokens remain host-only, are absent from logs and guest APIs, and are cleared on lock.
- Existing vaults and guest applications remain unchanged in behavior.
- Automated security tests and packaged cross-platform acceptance tests pass.
- Google and Microsoft setup documentation has been tested with registrations created by a maintainer.
- Limitations, recovery behavior, privacy, and provider-registration responsibilities are documented.

## 21. Deferred Decisions and Recommended Defaults

| Decision | Phase 2 default | Revisit when |
| --- | --- | --- |
| Is federation mandatory? | No; optional per vault. | A managed-enterprise policy model is designed. |
| Who owns provider registrations? | Distributor or administrator; bring your own client ID. | PixieVault accepts responsibility for official registrations. |
| Is a broker required? | No. | A hosted service provides enough concrete value to justify its trust and operating cost. |
| Are refresh tokens stored? | No. | Background API access or durable online sessions become a requirement. |
| Is offline federated authorization cached? | No; use local unlock. | A bounded offline enterprise policy has a defined revocation and risk model. |
| Can multiple external identities own a vault? | One owner in the initial UI; version the schema for future expansion. | Delegation, recovery contacts, or multi-user vaults are specified. |
| Are group claims mapped to roles? | No; deny unconfigured claims. | Enterprise authorization requirements and provider-specific semantics are known. |
| Facebook and Apple support? | Deferred. | Secure provider-specific adapters or an optional trusted backend are approved. |

## 22. Standards and Provider References

- [RFC 8252: OAuth 2.0 for Native Apps](https://www.rfc-editor.org/rfc/rfc8252)
- [RFC 7636: Proof Key for Code Exchange](https://www.rfc-editor.org/rfc/rfc7636)
- [RFC 9700: OAuth 2.0 Security Best Current Practice](https://www.rfc-editor.org/rfc/rfc9700)
- [OpenID Connect Core 1.0](https://openid.net/specs/openid-connect-core-1_0.html)
- [OpenID Connect Discovery 1.0](https://openid.net/specs/openid-connect-discovery-1_0.html)
- [Google OpenID Connect documentation](https://developers.google.com/identity/openid-connect/openid-connect)
- [Microsoft identity platform desktop application configuration](https://learn.microsoft.com/en-us/entra/identity-platform/scenario-desktop-app-configuration)
- [Apple Sign in with Apple token validation](https://developer.apple.com/documentation/signinwithapplerestapi/generate-and-validate-tokens)

This specification must be revalidated against the current standards and each provider's current registration requirements when implementation begins.
