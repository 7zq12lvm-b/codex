use std::sync::Arc;

use codex_agent_identity::AgentIdentityKey;
use codex_agent_identity::AgentTaskAuthorizationTarget;
use codex_agent_identity::authorization_header_for_agent_task;
use codex_api::AuthProvider;
use codex_api::Provider;
use codex_api::SharedAuthProvider;
use codex_login::AuthManager;
use codex_login::CodexAuth;
use codex_login::SsoConfig;
use codex_login::current_sso_env;
use codex_login::load_sso_session_for_env;
use codex_model_provider_info::ModelProviderInfo;
use http::HeaderMap;
use http::HeaderValue;

use crate::bearer_auth_provider::BearerAuthProvider;
use crate::cookie_auth_provider::CookieAuthProvider;

#[derive(Clone, Debug)]
struct AgentIdentityAuthProvider {
    auth: codex_login::auth::AgentIdentityAuth,
}

impl AuthProvider for AgentIdentityAuthProvider {
    fn add_auth_headers(&self, headers: &mut HeaderMap) {
        let record = self.auth.record();
        let header_value = self
            .auth
            .process_task_id()
            .ok_or_else(|| std::io::Error::other("agent identity process task is not initialized"))
            .and_then(|task_id| {
                authorization_header_for_agent_task(
                    AgentIdentityKey {
                        agent_runtime_id: &record.agent_runtime_id,
                        private_key_pkcs8_base64: &record.agent_private_key,
                    },
                    AgentTaskAuthorizationTarget {
                        agent_runtime_id: &record.agent_runtime_id,
                        task_id,
                    },
                )
                .map_err(std::io::Error::other)
            });

        if let Ok(header_value) = header_value
            && let Ok(header) = HeaderValue::from_str(&header_value)
        {
            let _ = headers.insert(http::header::AUTHORIZATION, header);
        }

        if let Ok(header) = HeaderValue::from_str(self.auth.account_id()) {
            let _ = headers.insert("ChatGPT-Account-ID", header);
        }

        if self.auth.is_fedramp_account() {
            let _ = headers.insert("X-OpenAI-Fedramp", HeaderValue::from_static("true"));
        }
    }
}

// Some providers are meant to send no auth headers. Examples include local OSS
// providers and custom test providers with `requires_openai_auth = false`.
#[derive(Clone, Debug)]
struct UnauthenticatedAuthProvider;

impl AuthProvider for UnauthenticatedAuthProvider {
    fn add_auth_headers(&self, _headers: &mut HeaderMap) {}
}

pub fn unauthenticated_auth_provider() -> SharedAuthProvider {
    Arc::new(UnauthenticatedAuthProvider)
}

fn sso_cookie(auth_manager: Option<&AuthManager>) -> Option<String> {
    let auth_manager = auth_manager?;
    let runtime_sso_env = current_sso_env();
    load_sso_session_for_env(auth_manager.codex_home(), runtime_sso_env)
        .ok()
        .flatten()
        .map(|session| session.cookie_header_value())
}

pub fn sso_auth_provider(auth_manager: Option<&AuthManager>) -> Option<SharedAuthProvider> {
    sso_cookie(auth_manager)
        .map(|cookie| Arc::new(CookieAuthProvider::new(cookie)) as SharedAuthProvider)
}

pub(crate) fn apply_sso_provider_override(
    provider: &ModelProviderInfo,
    auth_manager: Option<&AuthManager>,
    api_provider: &mut Provider,
) {
    if provider.requires_openai_auth && sso_cookie(auth_manager).is_some() {
        api_provider.base_url = SsoConfig::for_env(current_sso_env()).responses_api_base_url;
    }
}

/// Returns the provider-scoped auth manager when this provider uses command-backed auth.
///
/// Providers without custom auth continue using the caller-supplied base manager, when present.
pub(crate) fn auth_manager_for_provider(
    auth_manager: Option<Arc<AuthManager>>,
    provider: &ModelProviderInfo,
) -> Option<Arc<AuthManager>> {
    match provider.auth.clone() {
        Some(config) => Some(AuthManager::external_bearer_only(config)),
        None => auth_manager,
    }
}

pub(crate) fn resolve_provider_auth(
    auth: Option<&CodexAuth>,
    provider: &ModelProviderInfo,
) -> codex_protocol::error::Result<SharedAuthProvider> {
    if let Some(auth) = bearer_auth_for_provider(provider)? {
        return Ok(Arc::new(auth));
    }

    Ok(match auth {
        Some(auth) => auth_provider_from_auth(auth),
        None => unauthenticated_auth_provider(),
    })
}

fn bearer_auth_for_provider(
    provider: &ModelProviderInfo,
) -> codex_protocol::error::Result<Option<BearerAuthProvider>> {
    if let Some(api_key) = provider.api_key()? {
        return Ok(Some(BearerAuthProvider::new(api_key)));
    }

    if let Some(token) = provider.experimental_bearer_token.clone() {
        return Ok(Some(BearerAuthProvider::new(token)));
    }

    Ok(None)
}

/// Builds request-header auth for a first-party Codex auth snapshot.
pub fn auth_provider_from_auth(auth: &CodexAuth) -> SharedAuthProvider {
    match auth {
        CodexAuth::AgentIdentity(auth) => {
            Arc::new(AgentIdentityAuthProvider { auth: auth.clone() })
        }
        CodexAuth::ApiKey(_) | CodexAuth::Chatgpt(_) | CodexAuth::ChatgptAuthTokens(_) => {
            Arc::new(BearerAuthProvider {
                token: auth.get_token().ok(),
                account_id: auth.get_account_id(),
                is_fedramp_account: auth.is_fedramp_account(),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use codex_model_provider_info::WireApi;
    use codex_model_provider_info::create_oss_provider_with_base_url;

    use super::*;

    #[test]
    fn unauthenticated_auth_provider_adds_no_headers() {
        let provider =
            create_oss_provider_with_base_url("http://localhost:11434/v1", WireApi::Responses);
        let auth = resolve_provider_auth(/*auth*/ None, &provider).expect("auth should resolve");

        assert!(auth.to_auth_headers().is_empty());
    }
}
