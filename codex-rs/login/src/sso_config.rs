//! SSO environment configuration for company internal authentication.
//!
//! Provides per-environment (sit / prod) settings: login URL, ticket validation
//! endpoint, cookie name, and the public key used by backend gateways.

use std::fmt;

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
    /// Cookie name set by the SSO gateway on successful login.
    pub cookie_name: String,
    /// ECDSA public key (base64-encoded, SubjectPublicKeyInfo / DER) used by
    /// the gateway to sign `signedUserInfo`.
    pub pub_key: String,
    /// Subsystem alias registered with the SSO platform.
    pub subsystem_alias: String,
}

const SUBSYSTEM_ALIAS: &str = "codex";

impl SsoConfig {
    pub fn for_env(env: SsoEnv) -> Self {
        match env {
            SsoEnv::Sit => Self {
                env,
                login_url: "https://login2.sit.xiaohongshu.com/login".to_string(),
                validate_url: "https://login2.sit.xiaohongshu.com/sso/internal_login".to_string(),
                cookie_name: "common-internal-access-token-sit".to_string(),
                pub_key: "MFkwEwYHKoZIzj0CAQYIKoZIzj0DAQcDQgAE2iunWc0QXngFE/EWdN+CwZHDSScBWTjqnESruXGnZC+lpQYX7XZiObrPBz46bdRlAPhMCcXN3qIcFAXMslAPLQ==".to_string(),
                subsystem_alias: SUBSYSTEM_ALIAS.to_string(),
            },
            SsoEnv::Prod => Self {
                env,
                login_url: "https://login2.xiaohongshu.com/login".to_string(),
                validate_url: "https://login2.xiaohongshu.com/sso/internal_login".to_string(),
                cookie_name: "common-internal-access-token".to_string(),
                pub_key: "MFkwEwYHKoZIzj0CAQYIKoZIzj0DAQcDQgAE2iunWc0QXngFE/EWdN+CwZHDSScBWTjqnESruXGnZC+lpQYX7XZiObrPBz46bdRlAPhMCcXN3qIcFAXMslAPLQ==".to_string(),
                subsystem_alias: SUBSYSTEM_ALIAS.to_string(),
            },
        }
    }
}
