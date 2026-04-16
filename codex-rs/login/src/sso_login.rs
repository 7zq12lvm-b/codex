//! Company SSO login flow for the CLI.
//!
//! 1. Start a local HTTP server on a random port listening for `/callback`.
//! 2. Open the browser to the SSO login page with `service=http://localhost:{port}/callback`.
//! 3. On successful login the SSO platform redirects to the callback with `?ticket=ST-…`.
//! 4. The CLI POSTs the ticket to the internal validation endpoint (`/sso/internal_login`).
//! 5. On success the response body `data.accessToken` contains the token.
//!    We persist this along with user info to `~/.codex/sso_session.json`.

use std::io;
use std::net::TcpListener;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;

use serde::Deserialize;
use serde::Serialize;
use tiny_http::Request;
use tiny_http::Response;
use tiny_http::Server;
use tracing::error;
use tracing::info;

use crate::sso_config::SsoConfig;
use crate::sso_config::SsoEnv;

// ---------------------------------------------------------------------------
// Persisted session
// ---------------------------------------------------------------------------

/// User info returned by the SSO validation endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SsoUserInfo {
    pub user_id: String,
    pub name: String,
    pub display_name: String,
    pub email: String,
    #[serde(default)]
    pub email_alias: String,
    #[serde(default)]
    pub avatar: String,
}

/// On-disk representation stored at `$CODEX_HOME/sso_session_<env>.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SsoSession {
    /// The environment that produced this session (sit / prod).
    pub env: String,
    /// The access token returned by SSO (`data.accessToken`).
    pub access_token: String,
    /// All cookies from the `Set-Cookie` response headers, stored as
    /// `name=value` pairs. These should all be sent in subsequent requests.
    pub cookies: Vec<SsoCookieEntry>,
    /// User information from the SSO response.
    pub user: SsoUserInfo,
}

/// A single cookie extracted from a `Set-Cookie` header.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SsoCookieEntry {
    pub name: String,
    pub value: String,
}

impl SsoSession {
    /// Build a `Cookie` header value containing all persisted cookies,
    /// ready to attach to outgoing HTTP requests.
    pub fn cookie_header_value(&self) -> String {
        self.cookies
            .iter()
            .map(|c| format!("{}={}", c.name, c.value))
            .collect::<Vec<_>>()
            .join("; ")
    }
}

fn sso_session_path_for_env(codex_home: &Path, env: SsoEnv) -> PathBuf {
    codex_home.join(format!("sso_session_{env}.json"))
}

pub fn save_sso_session(codex_home: &Path, session: &SsoSession) -> io::Result<()> {
    let env = SsoEnv::from_str_loose(&session.env);
    let path = sso_session_path_for_env(codex_home, env);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(session).map_err(io::Error::other)?;

    let mut opts = std::fs::OpenOptions::new();
    opts.create(true).write(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    let mut file = opts.open(path)?;
    std::io::Write::write_all(&mut file, json.as_bytes())?;
    std::io::Write::flush(&mut file)?;
    Ok(())
}

pub fn load_sso_session(codex_home: &Path) -> io::Result<Option<SsoSession>> {
    let path = sso_session_path_for_env(codex_home, SsoEnv::Prod);
    match std::fs::read_to_string(&path) {
        Ok(contents) => {
            let session: SsoSession = serde_json::from_str(&contents).map_err(io::Error::other)?;
            Ok(Some(session))
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            let sit_path = sso_session_path_for_env(codex_home, SsoEnv::Sit);
            match std::fs::read_to_string(&sit_path) {
                Ok(contents) => {
                    let session: SsoSession =
                        serde_json::from_str(&contents).map_err(io::Error::other)?;
                    Ok(Some(session))
                }
                Err(sit_error) if sit_error.kind() == io::ErrorKind::NotFound => Ok(None),
                Err(sit_error) => Err(sit_error),
            }
        }
        Err(e) => Err(e),
    }
}

pub fn delete_sso_session(codex_home: &Path) -> io::Result<bool> {
    let mut removed = false;
    let paths = [
        sso_session_path_for_env(codex_home, SsoEnv::Prod),
        sso_session_path_for_env(codex_home, SsoEnv::Sit),
    ];
    for path in paths {
        match std::fs::remove_file(path) {
            Ok(()) => removed = true,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }
    Ok(removed)
}

// ---------------------------------------------------------------------------
// Local callback server
// ---------------------------------------------------------------------------

/// Handle for a running SSO callback server.
pub struct SsoLoginServer {
    /// The full SSO login URL the user should visit.
    pub login_url: String,
    /// The local port the callback server is listening on.
    pub actual_port: u16,
    server_handle: tokio::task::JoinHandle<io::Result<SsoSession>>,
}

impl SsoLoginServer {
    /// Block until the login flow completes and return the validated session.
    pub async fn wait_for_login(self) -> io::Result<SsoSession> {
        self.server_handle
            .await
            .map_err(|e| io::Error::other(format!("SSO login server panicked: {e:?}")))?
    }
}

/// Start the local callback server and return the browser URL + server handle.
pub fn start_sso_login(config: SsoConfig, codex_home: PathBuf) -> io::Result<SsoLoginServer> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let actual_port = listener.local_addr()?.port();
    let callback_url = format!("http://localhost:{actual_port}/callback");
    let login_url = format!(
        "{}?service={}",
        config.login_url,
        urlencoding::encode(&callback_url)
    );

    let server = Server::from_listener(listener, None).map_err(|e| {
        io::Error::other(format!("failed to create HTTP server from listener: {e}"))
    })?;
    let server = Arc::new(server);

    let (tx, mut rx) = tokio::sync::mpsc::channel::<Request>(4);
    {
        let server = server.clone();
        thread::spawn(move || {
            while let Ok(req) = server.recv() {
                if tx.blocking_send(req).is_err() {
                    break;
                }
            }
        });
    }

    let server_handle = {
        let server = server;
        tokio::spawn(async move {
            let result = loop {
                let Some(req) = rx.recv().await else {
                    break Err(io::Error::other("SSO callback channel closed unexpectedly"));
                };
                let url_raw = req.url().to_string();
                match handle_sso_request(&url_raw, &config, &callback_url, &codex_home).await {
                    RequestOutcome::Continue(resp) => {
                        let _ = tokio::task::spawn_blocking(move || req.respond(resp)).await;
                    }
                    RequestOutcome::Done {
                        response_body,
                        result,
                    } => {
                        let resp = Response::from_string(&response_body).with_status_code(200);
                        let _ = tokio::task::spawn_blocking(move || req.respond(resp)).await;
                        break result;
                    }
                }
            };
            server.unblock();
            result
        })
    };

    // Open the browser.
    let _ = webbrowser::open(&login_url);

    Ok(SsoLoginServer {
        login_url,
        actual_port,
        server_handle,
    })
}

enum RequestOutcome {
    Continue(Response<std::io::Cursor<Vec<u8>>>),
    Done {
        response_body: String,
        result: io::Result<SsoSession>,
    },
}

async fn handle_sso_request(
    url_raw: &str,
    config: &SsoConfig,
    callback_url: &str,
    codex_home: &Path,
) -> RequestOutcome {
    let parsed = match url::Url::parse(&format!("http://localhost{url_raw}")) {
        Ok(u) => u,
        Err(_) => {
            return RequestOutcome::Continue(
                Response::from_string("Bad Request").with_status_code(400),
            );
        }
    };

    if parsed.path() != "/callback" {
        return RequestOutcome::Continue(Response::from_string("Not Found").with_status_code(404));
    }

    let params: std::collections::HashMap<String, String> =
        parsed.query_pairs().into_owned().collect();

    let ticket = match params.get("ticket") {
        Some(t) if !t.is_empty() => t.clone(),
        _ => {
            return RequestOutcome::Done {
                response_body: "Missing ticket parameter".to_string(),
                result: Err(io::Error::other("SSO callback missing ticket parameter")),
            };
        }
    };

    info!("received SSO callback with ticket");

    match validate_ticket(config, &ticket, callback_url).await {
        Ok(session) => {
            if let Err(e) = save_sso_session(codex_home, &session) {
                error!("failed to persist SSO session: {e}");
                return RequestOutcome::Done {
                    response_body: format!("Login succeeded but failed to save session: {e}"),
                    result: Err(e),
                };
            }
            info!(
                "SSO login succeeded for {} ({})",
                session.user.display_name, session.user.email
            );
            RequestOutcome::Done {
                response_body: format!(
                    "Login successful! Welcome, {}. You can close this page.",
                    session.user.display_name
                ),
                result: Ok(session),
            }
        }
        Err(e) => {
            error!("SSO ticket validation failed: {e}");
            RequestOutcome::Done {
                response_body: format!("SSO login failed: {e}"),
                result: Err(e),
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Ticket validation
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct ValidateRequest {
    ticket: String,
    validate_service: String,
    subsystem_alias: String,
    set_global_domain: bool,
    token_used_as_universally: bool,
}

#[derive(Deserialize)]
struct ValidateResponse {
    code: i64,
    success: bool,
    msg: String,
    data: Option<ValidateResponseData>,
}

/// The `data` field returned by `/sso/internal_login`.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ValidateResponseData {
    access_token: String,
    user_id: String,
    name: String,
    display_name: String,
    email: String,
    #[serde(default)]
    email_alias: String,
    #[serde(default)]
    avatar: String,
}

async fn validate_ticket(
    config: &SsoConfig,
    ticket: &str,
    callback_url: &str,
) -> io::Result<SsoSession> {
    let body = ValidateRequest {
        ticket: ticket.to_string(),
        validate_service: callback_url.to_string(),
        subsystem_alias: config.subsystem_alias.clone(),
        set_global_domain: true,
        token_used_as_universally: true,
    };

    let client = reqwest::Client::new();
    let resp = client
        .post(&config.validate_url)
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| io::Error::other(format!("ticket validation request failed: {e}")))?;

    // Extract all Set-Cookie headers before consuming the body.
    let cookies = extract_all_cookies(&resp);

    let status = resp.status();
    let resp_body: ValidateResponse = resp
        .json()
        .await
        .map_err(|e| io::Error::other(format!("failed to parse validation response: {e}")))?;

    if !status.is_success() || !resp_body.success {
        return Err(io::Error::other(format!(
            "ticket validation failed (code={}): {}",
            resp_body.code, resp_body.msg
        )));
    }

    let data = resp_body
        .data
        .ok_or_else(|| io::Error::other("ticket validated but response data is missing"))?;

    Ok(SsoSession {
        env: config.env.to_string(),
        access_token: data.access_token,
        cookies,
        user: SsoUserInfo {
            user_id: data.user_id,
            name: data.name,
            display_name: data.display_name,
            email: data.email,
            email_alias: data.email_alias,
            avatar: data.avatar,
        },
    })
}

/// Extract all cookie name=value pairs from every `Set-Cookie` response header.
fn extract_all_cookies(resp: &reqwest::Response) -> Vec<SsoCookieEntry> {
    let mut cookies = Vec::new();
    for header_value in resp.headers().get_all(reqwest::header::SET_COOKIE) {
        let value_str = match header_value.to_str() {
            Ok(v) => v,
            Err(_) => continue,
        };
        // Set-Cookie: name=value; Path=/; Domain=...; ...
        // We only need the first segment before ';'.
        if let Some(kv) = value_str.split(';').next() {
            let mut parts = kv.splitn(2, '=');
            let name = parts.next().unwrap_or("").trim();
            let value = parts.next().unwrap_or("").trim();
            if !name.is_empty() {
                cookies.push(SsoCookieEntry {
                    name: name.to_string(),
                    value: value.to_string(),
                });
            }
        }
    }
    cookies
}
