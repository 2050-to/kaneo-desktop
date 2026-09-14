//! Instance discovery: asks a URL whether it speaks the Kaneo API.

use std::sync::LazyLock;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::instance_store::normalize_url;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Probe {
    pub url: String,
    pub has_users: bool,
    pub has_admin: bool,
    pub demo_mode: bool,
    pub registration_disabled: bool,
    /// Human-readable sign-in methods this instance offers.
    pub sign_in_methods: Vec<String>,
}

#[derive(Deserialize)]
struct Health {
    status: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct InstanceStatus {
    has_users: bool,
    has_admin: bool,
}

/// Subset of `GET /api/config` that tells the shell how a user can sign in.
/// Unknown fields are ignored so newer instances keep working.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct Config {
    disable_registration: bool,
    disable_email_otp_sign_in: bool,
    disable_login_form: bool,
    is_demo_mode: bool,
    has_smtp: bool,
    has_github_sign_in: bool,
    has_google_sign_in: bool,
    has_discord_sign_in: bool,
    has_custom_oauth: bool,
}

static CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| {
    reqwest::Client::builder()
        .timeout(REQUEST_TIMEOUT)
        .user_agent(concat!("kaneo-desktop/", env!("CARGO_PKG_VERSION")))
        .build()
        .expect("failed to build the HTTP client")
});

/// Mirrors `resolveApiBaseUrl` from Kaneo's `@kaneo/libs`.
pub fn api_base(instance_url: &str) -> String {
    let trimmed = instance_url.trim_end_matches('/');
    if trimmed.ends_with("/api") {
        trimmed.to_string()
    } else {
        format!("{trimmed}/api")
    }
}

pub async fn probe(input: &str) -> Result<Probe, String> {
    let url = normalize_url(input)?;
    let base = api_base(&url);

    let health = CLIENT
        .get(format!("{base}/health"))
        .send()
        .await
        .map_err(|error| unreachable(&url, &error))?;

    if !health.status().is_success() {
        return Err(format!(
            "{url} replied with HTTP {} — check that the URL points at a Kaneo instance.",
            health.status().as_u16()
        ));
    }

    let health: Health = health
        .json()
        .await
        .map_err(|_| format!("{url} did not answer with a Kaneo health response."))?;

    if health.status != "ok" {
        return Err(format!("{url} is reachable but its API is not healthy."));
    }

    let status = CLIENT
        .get(format!("{base}/instance/status"))
        .send()
        .await
        .map_err(|error| unreachable(&url, &error))?;

    if status.status() == reqwest::StatusCode::NOT_FOUND {
        return Err(format!(
            "{url} is reachable but serves no Kaneo API at {base}."
        ));
    }

    let status: InstanceStatus = status
        .json()
        .await
        .map_err(|_| format!("{url} answered with an unexpected instance status."))?;

    // `/config` is public and optional: instances older than the endpoint still work.
    let config = match CLIENT.get(format!("{base}/config")).send().await {
        Ok(response) if response.status().is_success() => response.json::<Config>().await.ok(),
        _ => None,
    };

    Ok(Probe {
        url,
        has_users: status.has_users,
        has_admin: status.has_admin,
        demo_mode: config.as_ref().is_some_and(|config| config.is_demo_mode),
        registration_disabled: config
            .as_ref()
            .is_some_and(|config| config.disable_registration),
        sign_in_methods: sign_in_methods(config.as_ref()),
    })
}

fn unreachable(url: &str, error: &reqwest::Error) -> String {
    if error.is_timeout() {
        format!("Timed out while contacting {url}.")
    } else {
        format!("Could not reach {url}: {error}")
    }
}

fn sign_in_methods(config: Option<&Config>) -> Vec<String> {
    let Some(config) = config else {
        return Vec::new();
    };

    let mut methods = Vec::new();

    if !config.disable_login_form {
        methods.push("Email & password".to_string());
    }
    if config.has_smtp && !config.disable_email_otp_sign_in {
        methods.push("Email sign-in code".to_string());
    }
    if config.has_github_sign_in {
        methods.push("GitHub".to_string());
    }
    if config.has_google_sign_in {
        methods.push("Google".to_string());
    }
    if config.has_discord_sign_in {
        methods.push("Discord".to_string());
    }
    if config.has_custom_oauth {
        methods.push("Custom OAuth".to_string());
    }

    methods
}

#[cfg(test)]
mod tests {
    use std::io::{BufRead, BufReader, Write};
    use std::net::{TcpListener, TcpStream};
    use std::thread;

    use super::*;

    /// Serves canned responses on a loopback port so probe tests never touch
    /// the network. Unmatched paths answer 404.
    fn serve(routes: Vec<(&'static str, u16, &'static str)>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").expect("failed to bind a test port");
        let port = listener.local_addr().expect("no local address").port();

        thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let routes = routes.clone();
                thread::spawn(move || respond(stream, &routes));
            }
        });

        format!("http://127.0.0.1:{port}")
    }

    fn respond(mut stream: TcpStream, routes: &[(&str, u16, &str)]) {
        let mut reader = BufReader::new(stream.try_clone().expect("failed to clone the stream"));

        let mut request_line = String::new();
        if reader.read_line(&mut request_line).is_err() {
            return;
        }

        loop {
            let mut header = String::new();
            match reader.read_line(&mut header) {
                Ok(0) | Err(_) => break,
                Ok(_) if header.trim().is_empty() => break,
                Ok(_) => {}
            }
        }

        let path = request_line.split_whitespace().nth(1).unwrap_or("/");
        let (status, body) = routes
            .iter()
            .find(|(route, _, _)| *route == path)
            .map(|(_, status, body)| (*status, *body))
            .unwrap_or((404, "{}"));

        let reason = if status == 200 { "OK" } else { "Not Found" };
        let response = format!(
            "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );

        let _ = stream.write_all(response.as_bytes());
        let _ = stream.flush();
    }

    fn probe_blocking(url: &str) -> Result<Probe, String> {
        tauri::async_runtime::block_on(probe(url))
    }

    #[test]
    fn appends_the_api_prefix_once() {
        assert_eq!(
            api_base("https://kaneo.example.com"),
            "https://kaneo.example.com/api"
        );
        assert_eq!(
            api_base("https://kaneo.example.com/"),
            "https://kaneo.example.com/api"
        );
        assert_eq!(
            api_base("https://kaneo.example.com/api"),
            "https://kaneo.example.com/api"
        );
    }

    #[test]
    fn probes_a_reachable_instance() {
        let base = serve(vec![
            ("/api/health", 200, r#"{"status":"ok"}"#),
            (
                "/api/instance/status",
                200,
                r#"{"hasUsers":true,"hasAdmin":true}"#,
            ),
            (
                "/api/config",
                200,
                r#"{"hasGithubSignIn":true,"disableLoginForm":false,"disableRegistration":true}"#,
            ),
        ]);

        let probe = probe_blocking(&base).expect("probe should succeed");

        assert_eq!(probe.url, base);
        assert!(probe.has_users);
        assert!(probe.registration_disabled);
        assert_eq!(
            probe.sign_in_methods,
            vec!["Email & password".to_string(), "GitHub".to_string()]
        );
    }

    #[test]
    fn rejects_a_host_that_serves_no_kaneo_api() {
        let base = serve(vec![("/api/health", 200, r#"{"status":"ok"}"#)]);

        let error = probe_blocking(&base).expect_err("probe should fail");

        assert!(
            error.contains("serves no Kaneo API"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn rejects_an_unhealthy_instance() {
        let base = serve(vec![("/api/health", 500, r#"{"status":"error"}"#)]);

        let error = probe_blocking(&base).expect_err("probe should fail");

        assert!(error.contains("HTTP 500"), "unexpected error: {error}");
    }

    #[test]
    fn reports_the_sign_in_methods_an_instance_enables() {
        let config = Config {
            has_smtp: true,
            has_github_sign_in: true,
            disable_login_form: true,
            ..Config::default()
        };

        assert_eq!(
            sign_in_methods(Some(&config)),
            vec!["Email sign-in code".to_string(), "GitHub".to_string()]
        );
    }

    #[test]
    fn hides_sign_in_methods_the_instance_turned_off() {
        let config = Config {
            disable_login_form: true,
            has_smtp: true,
            disable_email_otp_sign_in: true,
            ..Config::default()
        };

        assert!(sign_in_methods(Some(&config)).is_empty());
        assert!(sign_in_methods(None).is_empty());
    }

    #[test]
    fn serializes_to_camel_case_for_the_frontend() {
        let probe = Probe {
            url: "https://kaneo.example.com".to_string(),
            has_users: true,
            has_admin: false,
            demo_mode: false,
            registration_disabled: true,
            sign_in_methods: vec!["GitHub".to_string()],
        };

        let json = serde_json::to_value(probe).unwrap();

        assert_eq!(json["hasUsers"], true);
        assert_eq!(json["hasAdmin"], false);
        assert_eq!(json["registrationDisabled"], true);
        assert_eq!(json["signInMethods"][0], "GitHub");
    }
}
