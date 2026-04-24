//! CLI login commands backed by company SSO.
//!
//! `codex login`            – open browser for SSO login (default env: prod)
//! `codex login --env sit`  – use SIT SSO
//! `codex login status`     – check if a valid SSO session exists
//! `codex logout`           – remove persisted SSO session

use codex_core::config::Config;
use codex_login::SsoConfig;
use codex_login::SsoEnv;
use codex_login::delete_sso_session;
use codex_login::load_sso_session;
use codex_login::start_sso_login;
use codex_utils_cli::CliConfigOverrides;
use std::fs::OpenOptions;
use tracing_appender::non_blocking;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::Layer;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

/// Installs a small file-backed tracing layer for `codex login` diagnostics.
fn init_login_file_logging(config: &Config) -> Option<WorkerGuard> {
    let log_dir = match codex_core::config::log_dir(config) {
        Ok(log_dir) => log_dir,
        Err(err) => {
            eprintln!("Warning: failed to resolve login log directory: {err}");
            return None;
        }
    };

    if let Err(err) = std::fs::create_dir_all(&log_dir) {
        eprintln!(
            "Warning: failed to create login log directory {}: {err}",
            log_dir.display()
        );
        return None;
    }

    let mut log_file_opts = OpenOptions::new();
    log_file_opts.create(true).append(true);

    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        log_file_opts.mode(0o600);
    }

    let log_path = log_dir.join("codex-login.log");
    let log_file = match log_file_opts.open(&log_path) {
        Ok(f) => f,
        Err(err) => {
            eprintln!(
                "Warning: failed to open login log file {}: {err}",
                log_path.display()
            );
            return None;
        }
    };

    let (non_blocking, guard) = non_blocking(log_file);
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("codex_cli=info,codex_core=info,codex_login=info"));
    let file_layer = tracing_subscriber::fmt::layer()
        .with_writer(non_blocking)
        .with_target(true)
        .with_ansi(false)
        .with_filter(env_filter);

    if let Err(err) = tracing_subscriber::registry().with(file_layer).try_init() {
        eprintln!(
            "Warning: failed to initialize login log file {}: {err}",
            log_path.display()
        );
        return None;
    }

    Some(guard)
}

/// Run the SSO login flow: start local callback server, open browser, wait for ticket.
pub async fn run_sso_login(cli_config_overrides: CliConfigOverrides, env: SsoEnv) -> ! {
    let config = load_config_or_exit(cli_config_overrides).await;
    let _login_log_guard = init_login_file_logging(&config);
    tracing::info!("starting SSO login flow (env={env})");

    let sso_config = SsoConfig::for_env(env);
    let codex_home = config.codex_home.clone();

    match start_sso_login(sso_config, codex_home.to_path_buf()) {
        Ok(server) => {
            eprintln!(
                "Starting local SSO callback server on http://localhost:{}.\n\
                 If your browser did not open, navigate to this URL to authenticate:\n\n\
                 {}\n",
                server.actual_port, server.login_url
            );
            match server.wait_for_login().await {
                Ok(session) => {
                    eprintln!(
                        "Successfully logged in via SSO as {} ({})",
                        session.user.display_name, session.user.email
                    );
                    std::process::exit(0);
                }
                Err(e) => {
                    eprintln!("SSO login failed: {e}");
                    std::process::exit(1);
                }
            }
        }
        Err(e) => {
            eprintln!("Failed to start SSO login server: {e}");
            std::process::exit(1);
        }
    }
}

/// Show current SSO login status.
pub async fn run_login_status(cli_config_overrides: CliConfigOverrides) -> ! {
    let config = load_config_or_exit(cli_config_overrides).await;

    match load_sso_session(&config.codex_home) {
        Ok(Some(session)) => {
            eprintln!(
                "Logged in via SSO (env={}, user={}, email={})",
                session.env, session.user.display_name, session.user.email
            );
            std::process::exit(0);
        }
        Ok(None) => {
            eprintln!("Not logged in");
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!("Error checking login status: {e}");
            std::process::exit(1);
        }
    }
}

/// Remove persisted SSO session.
pub async fn run_logout(cli_config_overrides: CliConfigOverrides) -> ! {
    let config = load_config_or_exit(cli_config_overrides).await;

    match delete_sso_session(&config.codex_home) {
        Ok(true) => {
            eprintln!("Successfully logged out");
            std::process::exit(0);
        }
        Ok(false) => {
            eprintln!("Not logged in");
            std::process::exit(0);
        }
        Err(e) => {
            eprintln!("Error logging out: {e}");
            std::process::exit(1);
        }
    }
}

async fn load_config_or_exit(cli_config_overrides: CliConfigOverrides) -> Config {
    let cli_overrides = match cli_config_overrides.parse_overrides() {
        Ok(v) => v,
        Err(e) => {
            eprintln!("Error parsing -c overrides: {e}");
            std::process::exit(1);
        }
    };

    match Config::load_with_cli_overrides(cli_overrides).await {
        Ok(config) => config,
        Err(e) => {
            eprintln!("Error loading configuration: {e}");
            std::process::exit(1);
        }
    }
}
