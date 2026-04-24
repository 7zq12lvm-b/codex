use codex_api::AuthProvider;
use http::HeaderMap;
use http::HeaderValue;

#[derive(Clone, Debug)]
pub struct CookieAuthProvider {
    cookie: String,
}

impl CookieAuthProvider {
    pub fn new(cookie: String) -> Self {
        Self { cookie }
    }
}

impl AuthProvider for CookieAuthProvider {
    fn add_auth_headers(&self, headers: &mut HeaderMap) {
        if let Ok(header) = HeaderValue::from_str(&self.cookie) {
            let _ = headers.insert(http::header::COOKIE, header);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn cookie_auth_provider_reports_when_cookie_header_will_attach() {
        let auth = CookieAuthProvider::new("session=value".to_string());

        assert_eq!(
            codex_api::auth_header_telemetry(&auth),
            codex_api::AuthHeaderTelemetry {
                attached: true,
                name: Some("cookie"),
            }
        );
    }

    #[test]
    fn cookie_auth_provider_adds_cookie_header() {
        let auth = CookieAuthProvider::new("session=value".to_string());
        let mut headers = HeaderMap::new();

        auth.add_auth_headers(&mut headers);

        assert_eq!(
            headers
                .get(http::header::COOKIE)
                .and_then(|value| value.to_str().ok()),
            Some("session=value")
        );
    }
}
