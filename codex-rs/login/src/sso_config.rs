//! SSO environment configuration for company internal authentication.
//!
//! Provides per-environment (sit / prod) settings: login URL, ticket validation
//! endpoint, cookie name, and the public key used by backend gateways.

use std::fmt;
use std::sync::LazyLock;
use std::sync::RwLock;

/// SSO environment selector.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SsoEnv {
    Sit,
    Prod,
}

impl fmt::Display for SsoEnv {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sit => write!(f, "sit"),
            Self::Prod => write!(f, "prod"),
        }
    }
}

impl SsoEnv {
    pub fn from_str_loose(s: &str) -> Self {
        if s.eq_ignore_ascii_case("prod") || s.eq_ignore_ascii_case("production") {
            Self::Prod
        } else {
            Self::Sit
        }
    }
}

/// Resolved SSO configuration for a specific environment.
#[derive(Debug, Clone)]
pub struct SsoConfig {
    pub env: SsoEnv,
    /// Browser login URL, e.g. `https://login2.xiaohongshu.com/login`.
    pub login_url: String,
    /// Internal ticket validation endpoint.
    pub validate_url: String,
    /// Internal Responses API base URL selected for this SSO environment.
    pub responses_api_base_url: String,
    /// Cookie name set by the SSO gateway on successful login.
    pub cookie_name: String,
    /// ECDSA public key (base64-encoded, SubjectPublicKeyInfo / DER) used by
    /// the gateway to sign `signedUserInfo`.
    pub pub_key: String,
    /// Subsystem alias registered with the SSO platform.
    pub subsystem_alias: String,
}

const SUBSYSTEM_ALIAS: &str = "codex";
const CODEX_SSO_ENV_VAR: &str = "CODEX_SSO_ENV";
static CURRENT_SSO_ENV_OVERRIDE: LazyLock<RwLock<Option<SsoEnv>>> =
    LazyLock::new(|| RwLock::new(None));

impl SsoConfig {
    pub fn for_env(env: SsoEnv) -> Self {
        match env {
            SsoEnv::Sit => Self {
                env,
                login_url: "https://login2.sit.xiaohongshu.com/login".to_string(),
                validate_url: "https://login2.sit.xiaohongshu.com/sso/internal_login".to_string(),
                responses_api_base_url: "https://runway.devops.sit.xiaohongshu.com/openai/v2"
                    .to_string(),
                cookie_name: "common-internal-access-token-sit".to_string(),
                pub_key: "MFkwEwYHKoZIzj0CAQYIKoZIzj0DAQcDQgAE2iunWc0QXngFE/EWdN+CwZHDSScBWTjqnESruXGnZC+lpQYX7XZiObrPBz46bdRlAPhMCcXN3qIcFAXMslAPLQ==".to_string(),
                subsystem_alias: SUBSYSTEM_ALIAS.to_string(),
            },
            SsoEnv::Prod => Self {
                env,
                login_url: "https://login2.xiaohongshu.com/login".to_string(),
                validate_url: "https://login2.xiaohongshu.com/sso/internal_login".to_string(),
                responses_api_base_url: "https://runway.devops.xiaohongshu.com/openai/v2"
                    .to_string(),
                cookie_name: "common-internal-access-token".to_string(),
                pub_key: "MFkwEwYHKoZIzj0CAQYIKoZIzj0DAQcDQgAE2iunWc0QXngFE/EWdN+CwZHDSScBWTjqnESruXGnZC+lpQYX7XZiObrPBz46bdRlAPhMCcXN3qIcFAXMslAPLQ==".to_string(),
                subsystem_alias: SUBSYSTEM_ALIAS.to_string(),
            },
        }
    }
}

/// Sets the process-wide SSO environment used by runtime auth/session resolution.
pub fn set_current_sso_env(env: SsoEnv) {
    if let Ok(mut guard) = CURRENT_SSO_ENV_OVERRIDE.write() {
        *guard = Some(env);
    }
}

/// Returns the active runtime SSO environment.
///
/// Resolution order:
/// 1. Process override set via [`set_current_sso_env`]
/// 2. `CODEX_SSO_ENV` environment variable
/// 3. `prod` default
pub fn current_sso_env() -> SsoEnv {
    if let Ok(guard) = CURRENT_SSO_ENV_OVERRIDE.read()
        && let Some(env) = *guard
    {
        return env;
    }

    std::env::var(CODEX_SSO_ENV_VAR)
        .map(|value| SsoEnv::from_str_loose(&value))
        .unwrap_or(SsoEnv::Prod)
}
