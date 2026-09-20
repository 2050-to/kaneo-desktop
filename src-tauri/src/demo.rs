//! The bundled demo: a fake Kaneo server serving the real web app plus a
//! canned example workspace, entirely offline inside the app.
//!
//! The payload (`resources/web-demo.zip`) is the fork's production web build
//! with its API-URL placeholders rewritten to `http://127.0.0.1:41337`; this
//! module owns that port and answers the API from fixtures plus in-memory
//! state, so the demo can be clicked, dragged and edited until the app exits.

use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{json, Value};
use sha1_smol::Sha1;
use tauri::{AppHandle, Manager};
use zip::ZipArchive;

use crate::instance_store::Instance;

/// Fixed id for the built-in demo instance; a UUID, because macOS uses it as
/// the webview data-store identifier.
pub const DEMO_INSTANCE_ID: &str = "5dd2f7a9-3c1e-4f2b-9a0d-8e6c4b2a1d03";
pub const DEMO_NAME: &str = "Demo — Example Workspace";
/// The web payload was built with its API URL rewritten to this address.
pub const DEMO_PORT: u16 = 41337;
pub const DEMO_URL: &str = "http://127.0.0.1:41337";

const DEMO_EMAIL: &str = "demo@kaneo.desktop";
const DEMO_PASSWORD: &str = "demo-password";
const SESSION_COOKIE_TOKEN: &str = "better-auth.session_token=demo-session-452a9f";
const SESSION_COOKIE: &str =
    "better-auth.session_token=demo-session-452a9f; Path=/; HttpOnly; SameSite=Lax";
const SESSION_COOKIE_CLEAR: &str =
    "better-auth.session_token=; Path=/; Max-Age=0; Expires=Thu, 01 Jan 1970 00:00:00 GMT";
const WORKSPACE_ID: &str = "7c2e1a64-52b7-4c88-9d3f-2f8a5b1e7c02";
const PROJECT_ID: &str = "7d3b2c81-5e94-4f7a-b1c8-9d2e4f6a8b14";
const DEMO_USER_ID: &str = "c3a9f2e1-8b47-4d5a-9e6c-1d0f2a3b4c11";
const WS_GUID: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

const CONFIG: &str = include_str!("demo/fixtures/config.json");
const INSTANCE_STATUS: &str = include_str!("demo/fixtures/instance-status.json");
const SESSION: &str = include_str!("demo/fixtures/session.json");
const ORGANIZATIONS: &str = include_str!("demo/fixtures/organizations.json");
const FULL_ORGANIZATION: &str = include_str!("demo/fixtures/full-organization.json");
const ACTIVE_MEMBER: &str = include_str!("demo/fixtures/active-member.json");
const MEMBERS: &str = include_str!("demo/fixtures/members.json");
const LABELS: &str = include_str!("demo/fixtures/labels.json");
const BOARD: &str = include_str!("demo/fixtures/board.json");

/// Fills the demo credentials into the real sign-in form. The SPA mounts the
/// form after the initial HTML, so the script polls until the inputs exist.
const PREFILL_SCRIPT: &str = r#"<script>(() => {
  const EMAIL = "demo@kaneo.desktop", PASSWORD = "demo-password";
  const set = (el, value) => {
    const setter = Object.getOwnPropertyDescriptor(window.HTMLInputElement.prototype, "value").set;
    setter.call(el, value);
    el.dispatchEvent(new Event("input", { bubbles: true }));
  };
  const tries = setInterval(() => {
    if (!location.pathname.startsWith("/auth/sign-in")) return;
    const email = document.querySelector('input[name="email"]');
    const password = document.querySelector('input[name="password"]');
    if (email && password) {
      if (!email.value) set(email, EMAIL);
      if (!password.value) set(password, PASSWORD);
      clearInterval(tries);
    }
  }, 400);
  setTimeout(() => clearInterval(tries), 20000);
})();</script>"#;

struct DemoState {
    board: Value,
    labels: Vec<Value>,
    comments: HashMap<String, Vec<Value>>,
    activities: HashMap<String, Vec<Value>>,
    time_entries: HashMap<String, Vec<Value>>,
}

struct DemoRuntime {
    state: Mutex<DemoState>,
    payload: Mutex<ZipArchive<File>>,
}

static RUNTIME: OnceLock<DemoRuntime> = OnceLock::new();
static STARTED: Mutex<bool> = Mutex::new(false);

/// The demo instance the launcher opens, with its own webview data store.
pub fn demo_instance() -> Instance {
    Instance {
        id: DEMO_INSTANCE_ID.to_string(),
        name: DEMO_NAME.to_string(),
        url: DEMO_URL.to_string(),
    }
}

/// Starts the demo server once. Later calls are cheap `Ok`s while it runs.
pub fn start(app: &AppHandle) -> Result<(), String> {
    let mut started = STARTED
        .lock()
        .map_err(|_| "The demo server is shutting down.".to_string())?;
    if *started {
        return Ok(());
    }

    let payload = open_payload(app)?;
    let state = build_state()?;
    let listener = TcpListener::bind(("127.0.0.1", DEMO_PORT))
        .map_err(|error| format!("The demo server port ({DEMO_PORT}) is busy: {error}"))?;

    RUNTIME
        .set(DemoRuntime {
            state: Mutex::new(state),
            payload: Mutex::new(payload),
        })
        .map_err(|_| "The demo server was already running.".to_string())?;

    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(stream) = stream else { continue };
            std::thread::spawn(move || {
                let Some(runtime) = RUNTIME.get() else { return };
                let _ = handle_connection(runtime, stream);
            });
        }
    });

    *started = true;
    Ok(())
}

fn build_state() -> Result<DemoState, String> {
    let mut state = DemoState {
        board: serde_json::from_str(BOARD).map_err(|e| format!("Corrupt board fixture: {e}"))?,
        labels: serde_json::from_str(LABELS).map_err(|e| format!("Corrupt labels fixture: {e}"))?,
        comments: HashMap::new(),
        activities: HashMap::new(),
        time_entries: HashMap::new(),
    };

    // A flagship task carries the demo's depth: a conversation, a paper trail
    // and a finished timer, so the task detail reads like a used board.
    state.comments.insert(
        "t2-pricing-layout".to_string(),
        vec![
            json!({
                "id": "cm-1", "taskId": "t2-pricing-layout",
                "userId": "b8d4e7c2-9f13-4a86-bd2e-5c7f9a1e3d12",
                "content": "I pulled the copy from the doc — the three-tier grid is final, only the enterprise card needs a decision.",
                "createdAt": "2026-09-09T10:15:00.000Z", "updatedAt": "2026-09-09T10:15:00.000Z",
                "user": { "name": "Avery Cole", "image": null }
            }),
            json!({
                "id": "cm-2", "taskId": "t2-pricing-layout",
                "userId": DEMO_USER_ID,
                "content": "Ship the tiers first. We can decide the enterprise card once the layout holds up on mobile.",
                "createdAt": "2026-09-09T11:02:00.000Z", "updatedAt": "2026-09-09T11:02:00.000Z",
                "user": { "name": "Demo User", "image": null }
            }),
        ],
    );
    state.activities.insert(
        "t2-pricing-layout".to_string(),
        vec![
            json!({
                "id": "act-1", "taskId": "t2-pricing-layout", "type": "create",
                "content": "created this task", "eventData": null,
                "createdAt": "2026-09-02T12:00:00.000Z", "updatedAt": "2026-09-02T12:00:00.000Z",
                "userId": DEMO_USER_ID, "externalUserName": null, "externalUserAvatar": null,
                "externalSource": null, "externalUrl": null
            }),
            json!({
                "id": "act-2", "taskId": "t2-pricing-layout", "type": "status_changed",
                "content": "changed status from To Do to In Progress",
                "eventData": { "oldStatus": "to-do", "newStatus": "in-progress" },
                "createdAt": "2026-09-08T09:30:00.000Z", "updatedAt": "2026-09-08T09:30:00.000Z",
                "userId": DEMO_USER_ID, "externalUserName": null, "externalUserAvatar": null,
                "externalSource": null, "externalUrl": null
            }),
        ],
    );
    state.time_entries.insert(
        "t2-pricing-layout".to_string(),
        vec![json!({
            "id": "te-1", "taskId": "t2-pricing-layout", "userId": DEMO_USER_ID,
            "description": "Grid layout + responsive pass",
            "startTime": "2026-09-08T09:00:00.000Z", "endTime": "2026-09-08T11:30:00.000Z",
            "duration": 9000,
            "createdAt": "2026-09-08T09:00:00.000Z", "updatedAt": "2026-09-08T11:30:00.000Z",
            "userName": "Demo User"
        })],
    );

    Ok(state)
}

fn open_payload(app: &AppHandle) -> Result<ZipArchive<File>, String> {
    let mut candidates: Vec<PathBuf> = Vec::new();

    if let Ok(resource_dir) = app.path().resource_dir() {
        candidates.push(resource_dir.join("web-demo.zip"));
        candidates.push(resource_dir.join("resources").join("web-demo.zip"));
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.join("resources").join("web-demo.zip"));
        }
    }
    if let Ok(manifest) = std::env::var("CARGO_MANIFEST_DIR") {
        candidates.push(
            PathBuf::from(manifest)
                .join("resources")
                .join("web-demo.zip"),
        );
    }

    for candidate in candidates {
        if let Ok(file) = File::open(&candidate) {
            return ZipArchive::new(file).map_err(|error| format!("Corrupt demo payload: {error}"));
        }
    }

    Err("The demo payload is missing from this install.".to_string())
}

// --- HTTP plumbing ---------------------------------------------------------

struct Request {
    method: String,
    path: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl Request {
    fn header(&self, name: &str) -> Option<&str> {
        let name = name.to_ascii_lowercase();
        self.headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(&name))
            .map(|(_, value)| value.as_str())
    }

    fn has_session(&self) -> bool {
        self.header("cookie")
            .is_some_and(|cookies| cookies.contains(SESSION_COOKIE_TOKEN))
    }

    fn json_body(&self) -> Value {
        serde_json::from_slice(&self.body).unwrap_or_else(|_| json!({}))
    }
}

struct Response {
    status: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl Response {
    fn json(status: u16, value: &Value) -> Response {
        Response {
            status,
            headers: vec![("Content-Type".to_string(), "application/json".to_string())],
            body: serde_json::to_vec(value).unwrap_or_else(|_| b"{}".to_vec()),
        }
    }

    fn text(status: u16, content_type: &str, body: Vec<u8>) -> Response {
        Response {
            status,
            headers: vec![("Content-Type".to_string(), content_type.to_string())],
            body,
        }
    }

    fn empty(status: u16) -> Response {
        Response {
            status,
            headers: Vec::new(),
            body: Vec::new(),
        }
    }

    fn header(mut self, name: &str, value: &str) -> Response {
        self.headers.push((name.to_string(), value.to_string()));
        self
    }

    fn with_cookie(self, cookie: &str) -> Response {
        self.header("Set-Cookie", cookie)
    }
}

fn handle_connection(runtime: &DemoRuntime, mut stream: TcpStream) -> std::io::Result<()> {
    let Some(request) = read_request(&mut stream)? else {
        return Ok(());
    };

    // The SPA keeps live-update sockets per project and per user. A held-open
    // socket is enough: the client retries silently if it closes, and nothing
    // broadcasts in a single-user demo.
    if wants_websocket(&request) {
        return accept_websocket(&mut stream, &request);
    }

    let response = route(runtime, &request);
    write_response(&mut stream, &response)
}

fn wants_websocket(request: &Request) -> bool {
    request.path.starts_with("/api/ws/")
        && request
            .header("upgrade")
            .is_some_and(|upgrade| upgrade.eq_ignore_ascii_case("websocket"))
}

fn accept_websocket(stream: &mut TcpStream, request: &Request) -> std::io::Result<()> {
    let key = request.header("sec-websocket-key").unwrap_or_default();
    let accept = {
        let mut hash = Sha1::new();
        hash.update(key.as_bytes());
        hash.update(WS_GUID.as_bytes());
        base64(&hash.digest().bytes())
    };

    let upgrade = format!(
        "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {accept}\r\n\r\n"
    );
    stream.write_all(upgrade.as_bytes())?;
    stream.flush()?;

    // Park the socket: the client's app-level pings stay unread in the
    // buffer, and the socket closes when the webview drops the connection.
    let mut seen = [0u8; 4096];
    while let Ok(n) = stream.read(&mut seen) {
        if n == 0 {
            break;
        }
    }

    Ok(())
}

fn read_request(stream: &mut TcpStream) -> std::io::Result<Option<Request>> {
    let mut buffer: Vec<u8> = Vec::new();
    let mut chunk = [0u8; 4096];

    let header_end = loop {
        if let Some(position) = find_window(&buffer, b"\r\n\r\n") {
            break position;
        }
        let read = stream.read(&mut chunk)?;
        if read == 0 {
            return Ok(None);
        }
        buffer.extend_from_slice(&chunk[..read]);
    };

    let head = String::from_utf8_lossy(&buffer[..header_end]).into_owned();
    let mut lines = head.split("\r\n");
    let request_line = lines.next().unwrap_or_default().to_string();
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or_default().to_uppercase();
    let target = parts.next().unwrap_or("/").to_string();
    let path = target.split('?').next().unwrap_or("/").to_string();

    let mut headers = Vec::new();
    for line in lines {
        if let Some((name, value)) = line.split_once(':') {
            headers.push((name.trim().to_string(), value.trim().to_string()));
        }
    }

    let content_length = headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("content-length"))
        .and_then(|(_, value)| value.parse::<usize>().ok())
        .unwrap_or(0);

    let mut body = buffer[header_end + 4..].to_vec();
    while body.len() < content_length {
        let read = stream.read(&mut chunk)?;
        if read == 0 {
            break;
        }
        body.extend_from_slice(&chunk[..read]);
    }
    body.truncate(content_length);

    Ok(Some(Request {
        method,
        path,
        headers,
        body,
    }))
}

fn mime_for(path: &str) -> &'static str {
    match path.rsplit('.').next() {
        Some("html") => "text/html; charset=utf-8",
        Some("js") => "text/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("json") | Some("webmanifest") => "application/json",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("ico") => "image/x-icon",
        Some("woff2") => "font/woff2",
        Some("woff") => "font/woff",
        _ => "application/octet-stream",
    }
}

fn find_window(buffer: &[u8], window: &[u8]) -> Option<usize> {
    buffer
        .windows(window.len())
        .position(|slice| slice == window)
}

fn write_response(stream: &mut TcpStream, response: &Response) -> std::io::Result<()> {
    let reason = match response.status {
        200 => "OK",
        201 => "Created",
        101 => "Switching Protocols",
        401 => "Unauthorized",
        404 => "Not Found",
        _ => "Error",
    };
    let mut head = format!("HTTP/1.1 {} {reason}\r\n", response.status);
    let mut headers = response.headers.clone();
    headers.push((
        "Content-Length".to_string(),
        response.body.len().to_string(),
    ));
    headers.push(("Connection".to_string(), "close".to_string()));
    for (name, value) in headers {
        head.push_str(&format!("{name}: {value}\r\n"));
    }
    head.push_str("\r\n");

    stream.write_all(head.as_bytes())?;
    stream.write_all(&response.body)?;
    stream.flush()
}

fn now_iso() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let seconds = now.as_secs() as i64;
    let millis = now.subsec_millis();
    let days = seconds.div_euclid(86_400);
    let secs_of_day = seconds.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days.div_euclid(719_468));
    let (hour, minute, second) = (
        secs_of_day / 3600,
        (secs_of_day % 3600) / 60,
        secs_of_day % 60,
    );
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}.{millis:03}Z")
}

/// Howard Hinnant's `civil_from_days`, for UTC timestamps without a crate.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doe - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m as u32, d as u32)
}

fn base64(input: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in input.chunks(3) {
        let packed = ((chunk[0] as u32) << 16)
            | ((chunk.get(1).copied().unwrap_or(0) as u32) << 8)
            | chunk.get(2).copied().unwrap_or(0) as u32;
        out.push(TABLE[(packed >> 18) as usize & 63] as char);
        out.push(TABLE[(packed >> 12) as usize & 63] as char);
        if chunk.len() > 1 {
            out.push(TABLE[(packed >> 6) as usize & 63] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(TABLE[packed as usize & 63] as char);
        } else {
            out.push('=');
        }
    }
    out
}

// --- Router ----------------------------------------------------------------

fn route(runtime: &DemoRuntime, request: &Request) -> Response {
    if request.path == "/api" || request.path.starts_with("/api/") {
        return route_api(runtime, request);
    }
    serve_static(runtime, request)
}

fn serve_static(runtime: &DemoRuntime, request: &Request) -> Response {
    let mut path = request.path.trim_start_matches('/').to_string();
    if path.is_empty() {
        path = "index.html".to_string();
    }
    let has_extension = path
        .rsplit('.')
        .next()
        .is_some_and(|extension| !extension.is_empty() && extension.len() <= 5);

    let entry = if has_extension {
        path.clone()
    } else {
        "index.html".to_string()
    };

    let served = {
        let mut payload = match runtime.payload.lock() {
            Ok(payload) => payload,
            Err(_) => return Response::empty(500),
        };
        payload
            .by_name(&entry)
            .ok()
            .map(|mut file| {
                let mut body = Vec::with_capacity(file.size() as usize);
                let _ = file.read_to_end(&mut body);
                body
            })
            .map(|body| (entry.clone(), body))
    };

    let Some((entry, body)) = served else {
        return Response::json(404, &json!({ "message": "Not found" }));
    };

    let content_type = mime_for(&entry).to_string();
    let cache = if entry.starts_with("assets/") {
        "public, max-age=31536000, immutable"
    } else {
        "no-store"
    };

    if entry == "index.html" {
        let html = String::from_utf8_lossy(&body).into_owned();
        let injected = html.replace("</body>", &format!("{PREFILL_SCRIPT}</body>"));
        Response::text(200, "text/html; charset=utf-8", injected.into_bytes())
            .header("Cache-Control", "no-store")
    } else {
        Response::text(200, &content_type, body).header("Cache-Control", cache)
    }
}

/// Paths the SPA may call before it has a session.
fn is_public_api(path: &str) -> bool {
    path == "/api/config"
        || path == "/api/instance/status"
        || path.starts_with("/api/auth/")
        || path.starts_with("/api/public-project/")
}

fn route_api(runtime: &DemoRuntime, request: &Request) -> Response {
    if request.method == "OPTIONS" {
        return Response::empty(204)
            .header("Access-Control-Allow-Origin", DEMO_URL)
            .header("Access-Control-Allow-Credentials", "true");
    }

    if !is_public_api(request.path.as_str()) && !request.has_session() {
        return Response::json(401, &json!({ "message": "Unauthorized" }));
    }

    let segments: Vec<&str> = request
        .path
        .trim_start_matches("/api/")
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect();
    let Ok(mut state) = runtime.state.lock() else {
        return Response::empty(500);
    };

    api(&mut state, request, &segments)
}

fn fixture(value: &str) -> Value {
    serde_json::from_str(value).unwrap_or(json!(null))
}

fn member_name(user_id: &Value) -> Option<&'static str> {
    match user_id.as_str() {
        Some(DEMO_USER_ID) => Some("Demo User"),
        Some("b8d4e7c2-9f13-4a86-bd2e-5c7f9a1e3d12") => Some("Avery Cole"),
        Some("e4f1a9b8-2c75-4d3e-9a8f-6b1d3e7c5a13") => Some("Sam Rivera"),
        _ => None,
    }
}

fn assignee_fields(task: &mut Value) {
    let user_id = task.get("userId").cloned().unwrap_or(Value::Null);
    let name = member_name(&user_id);
    task["assigneeName"] = json!(name);
    task["assigneeId"] = user_id;
    task["assigneeImage"] = Value::Null;
}

fn not_found() -> Response {
    Response::json(404, &json!({ "message": "Not found" }))
}

fn forbidden() -> Response {
    Response::json(403, &json!({ "message": "Forbidden" }))
}

fn api(state: &mut DemoState, request: &Request, segments: &[&str]) -> Response {
    let method = request.method.as_str();

    // Better-auth session and organization routes are a fixed dialect.
    if segments.first() == Some(&"auth") {
        return auth(state, request, segments);
    }

    match (method, segments) {
        ("GET", ["config"]) => Response::json(200, &fixture(CONFIG)),
        ("GET", ["instance", "status"]) => Response::json(200, &fixture(INSTANCE_STATUS)),
        ("GET", ["invitation", "pending"]) => Response::json(200, &json!([])),
        ("GET", ["notification"]) => Response::json(200, &json!([])),
        ("GET", ["search"]) => Response::json(200, &json!([])),
        ("GET", ["label", "workspace", _workspace]) => {
            Response::json(200, &Value::Array(state.labels.clone()))
        }

        ("GET", ["project"]) => Response::json(200, &projects_payload(state)),
        ("POST", ["project"]) => create_project(state, request),
        ("PATCH", ["project", "reorder"]) => Response::json(200, &json!({})),
        ("GET", ["project", id]) => project_payload(state, id),
        ("PATCH", ["project", id]) => update_project(state, id, request),
        ("DELETE", ["project", id]) => delete_project(state, id),

        ("GET", ["task", "tasks", project_id]) => board_payload(state, project_id),
        ("POST", ["task", "move", id]) => move_task(state, id, request),
        ("POST", ["task", project_id]) => create_task(state, project_id, request),
        ("PATCH", ["task", "bulk"]) => bulk_tasks(state, request),
        ("GET", ["task", id]) => single_task(state, id),
        ("PUT", ["task", id]) | ("PATCH", ["task", id]) => update_task(state, id, request),
        ("DELETE", ["task", id]) => delete_task(state, id),
        ("PATCH", ["task", "status", id]) => patch_task_field(state, id, "status", request),
        ("PATCH", ["task", "priority", id]) => patch_task_field(state, id, "priority", request),
        ("PATCH", ["task", "assignee", id]) => patch_task_field(state, id, "userId", request),
        ("PATCH", ["task", "due-date", id]) => patch_task_field(state, id, "dueDate", request),
        ("PATCH", ["task", "title", id]) => patch_task_field(state, id, "title", request),
        ("PATCH", ["task", "description", id]) => {
            patch_task_field(state, id, "description", request)
        }

        ("POST", ["column", _project_id]) => create_column(state, request),
        ("PATCH", ["column", id]) => patch_column(state, id, request),
        ("POST", ["column", "reorder", _project_id]) => Response::json(200, &json!({})),
        ("DELETE", ["column", id]) => delete_column(state, id),

        ("GET", ["comment", task_id]) => comments_payload(state, task_id),
        ("POST", ["comment", task_id]) => create_comment(state, task_id, request),
        ("PATCH", ["comment", id]) => update_comment(state, id, request),
        ("DELETE", ["comment", id]) => delete_comment(state, id),

        ("GET", ["activity", task_id]) => activities_payload(state, task_id),
        ("POST", ["activity", "create"]) | ("POST", ["activity", "comment"]) => {
            create_activity(state, request)
        }

        ("GET", ["label", "task", task_id]) => task_labels_payload(state, task_id),
        ("POST", ["label"]) => create_label(state, request),
        ("PATCH", ["label", id]) => patch_label(state, id, request),
        ("DELETE", ["label", id]) => delete_label(state, id),
        ("POST", ["label", id, "task"]) => attach_label(state, id, request),
        ("DELETE", ["label", id, "task"]) => detach_label(state, id, request),

        ("GET", ["time-entry", "task", task_id]) => time_entries_payload(state, task_id),
        ("POST", ["time-entry"]) => create_time_entry(state, request),

        ("GET", ["external-link", _task_id]) => Response::json(200, &json!([])),
        ("GET", ["task-relation", _task_id]) => Response::json(200, &json!([])),
        ("GET", ["custom-field", _rest @ ..]) => Response::json(200, &json!([])),

        ("PATCH", ["notification", _rest @ ..]) => Response::json(200, &json!({})),
        ("DELETE", ["notification", _rest @ ..]) => Response::json(200, &json!({})),

        _ => not_found(),
    }
}

// --- Better-auth ------------------------------------------------------------

fn auth(_state: &mut DemoState, request: &Request, segments: &[&str]) -> Response {
    match (request.method.as_str(), segments) {
        ("GET", ["auth", "get-session"]) => {
            if request.has_session() {
                Response::json(200, &fixture(SESSION))
            } else {
                Response::json(200, &Value::Null)
            }
        }
        ("POST", ["auth", "sign-in", "email"]) => sign_in(request),
        ("POST", ["auth", "sign-out"]) => {
            Response::json(200, &json!({ "success": true })).with_cookie(SESSION_COOKIE_CLEAR)
        }
        ("GET", ["auth", "organization", "list-organizations"]) => {
            Response::json(200, &fixture(ORGANIZATIONS))
        }
        ("GET", ["auth", "organization", "list"]) => Response::json(200, &fixture(ORGANIZATIONS)),
        ("GET", ["auth", "organization", "get-active-organization"]) => {
            Response::json(200, &fixture(FULL_ORGANIZATION))
        }
        ("GET", ["auth", "organization", "get-full-organization"]) => {
            Response::json(200, &fixture(FULL_ORGANIZATION))
        }
        ("GET", ["auth", "organization", "get-active-member"]) => {
            Response::json(200, &fixture(ACTIVE_MEMBER))
        }
        ("GET", ["auth", "organization", "list-members"]) => Response::json(200, &fixture(MEMBERS)),
        ("POST", ["auth", "organization", "has-permission"]) => {
            Response::json(200, &json!({ "success": true }))
        }
        ("GET", ["auth", "organization", "list-invitations"])
        | ("GET", ["auth", "organization", "list-user-invitations"]) => {
            Response::json(200, &json!([]))
        }
        ("POST", ["auth", "organization", _rest @ ..]) => Response::json(200, &json!({})),
        ("GET", ["auth", _rest @ ..]) => Response::json(200, &Value::Null),
        _ => not_found(),
    }
}

fn sign_in(request: &Request) -> Response {
    let body = request.json_body();
    let email = body
        .get("email")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let password = body
        .get("password")
        .and_then(Value::as_str)
        .unwrap_or_default();

    if email != DEMO_EMAIL || password != DEMO_PASSWORD {
        return Response::json(
            401,
            &json!({ "message": "Invalid email or password. The demo login is printed on the launcher card." }),
        );
    }

    Response::json(
        200,
        &json!({ "token": "demo-session-452a9f", "user": fixture(SESSION).get("user") }),
    )
    .with_cookie(SESSION_COOKIE)
}

// --- Read payloads ----------------------------------------------------------

fn board_payload(state: &mut DemoState, project_id: &str) -> Response {
    if project_id != PROJECT_ID {
        return not_found();
    }
    let mut board = state.board.clone();
    for column in board["data"]["columns"]
        .as_array_mut()
        .into_iter()
        .flatten()
    {
        if let Some(tasks) = column.get_mut("tasks").and_then(Value::as_array_mut) {
            for task in tasks.iter_mut() {
                assignee_fields(task);
            }
            // The UI renders in list order; keep it aligned with positions so
            // same-column reorder drops land exactly where the user dropped.
            tasks.sort_by_key(|task| task["position"].as_u64().unwrap_or(u64::MAX));
        }
    }
    Response::json(200, &board)
}

fn projects_payload(state: &DemoState) -> Value {
    let mut projects = json!([{
        "id": PROJECT_ID,
        "workspaceId": WORKSPACE_ID,
        "slug": "WEB",
        "icon": "Globe",
        "name": "Website Redesign",
        "description": "The full revamp: marketing site, pricing, blog and docs.",
        "createdAt": "2026-09-02T08:00:00.000Z",
        "isPublic": false,
        "archivedAt": null,
        "position": 1000,
        "lastTaskNumber": 9,
        "statistics": { "completionPercentage": 22, "totalTasks": 9, "dueDate": "2027-02-28T17:00:00.000Z" },
        "archivedTasks": [], "plannedTasks": [], "columns": []
    }]);

    if let Some(item) = projects.get_mut(0) {
        item["statistics"] = project_statistics(state);
        item["lastTaskNumber"] = json!(highest_task_number(state).unwrap_or(9));
    }
    projects
}

fn highest_task_number(state: &DemoState) -> Option<u64> {
    state.board["data"]["columns"]
        .as_array()
        .map(|columns| {
            columns
                .iter()
                .flat_map(|column| column["tasks"].as_array().cloned().unwrap_or_default())
                .filter_map(|task| task["number"].as_u64())
                .max()
        })
        .and_then(|inner| inner)
}

fn project_statistics(state: &DemoState) -> Value {
    let mut total = 0u64;
    let mut done = 0u64;
    let mut soonest: Option<String> = None;
    for column in state.board["data"]["columns"]
        .as_array()
        .into_iter()
        .flatten()
    {
        let is_final = column["isFinal"].as_bool().unwrap_or(false);
        for task in column["tasks"].as_array().into_iter().flatten() {
            total += 1;
            if is_final {
                done += 1;
            }
            if let Some(due) = task["dueDate"].as_str() {
                soonest = Some(match soonest {
                    Some(best) if best.as_str() <= due => best,
                    _ => due.to_string(),
                });
            }
        }
    }
    let completion = (done * 100).checked_div(total).unwrap_or(0);
    json!({
        "completionPercentage": completion,
        "totalTasks": total,
        "dueDate": soonest,
    })
}

fn project_payload(state: &DemoState, id: &str) -> Response {
    if id != PROJECT_ID {
        return not_found();
    }
    let projects = projects_payload(state);
    match projects.as_array().and_then(|list| list.first()) {
        Some(project) => {
            let mut detail = project.clone();
            if let Some(object) = detail.as_object_mut() {
                for key in ["statistics", "archivedTasks", "plannedTasks", "columns"] {
                    object.remove(key);
                }
            }
            Response::json(
                200,
                &Value::Object(detail.as_object().cloned().unwrap_or_default()),
            )
        }
        None => not_found(),
    }
}

// --- Task mutations ---------------------------------------------------------

fn find_task_mut<'a>(board: &'a mut Value, id: &str) -> Option<&'a mut Value> {
    let columns = board.get_mut("data")?.get_mut("columns")?.as_array_mut()?;
    columns.iter_mut().find_map(|column| {
        column
            .get_mut("tasks")?
            .as_array_mut()?
            .iter_mut()
            .find(|task| task["id"] == *id)
    })
}

/// After a task's status changed, the column arrays have to agree with it:
/// pull the task out of whichever column holds it and append it to the
/// destination column at the next landing position.
fn apply_status_change(state: &mut DemoState, id: &str, destination: &str) -> Option<Value> {
    let landing = next_position(&state.board, destination);
    let mut moved: Option<Value> = None;
    let columns = state
        .board
        .get_mut("data")
        .and_then(|data| data.get_mut("columns"))
        .and_then(Value::as_array_mut);
    if let Some(columns) = columns {
        for column in columns.iter_mut() {
            let Some(tasks) = column.get_mut("tasks").and_then(Value::as_array_mut) else {
                continue;
            };
            if let Some(position) = tasks.iter().position(|task| task["id"] == id) {
                let mut task = tasks.remove(position);
                task["status"] = json!(destination);
                task["position"] = json!(landing);
                moved = Some(task);
                break;
            }
        }
    }

    let task = moved?;
    if let Some(target) = column_mut(&mut state.board, destination) {
        if let Some(tasks) = target.get_mut("tasks").and_then(Value::as_array_mut) {
            tasks.push(task.clone());
        }
    }
    Some(task)
}

fn single_task(state: &DemoState, id: &str) -> Response {
    let columns = state.board["data"]["columns"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let task = columns
        .iter()
        .flat_map(|column| column["tasks"].as_array().cloned().unwrap_or_default())
        .find(|task| task["id"].as_str() == Some(id));

    match task {
        Some(mut task) => {
            assignee_fields(&mut task);
            task["projectId"] = json!(PROJECT_ID);
            // taskSchema detail shape: a subset of the board task.
            let mut detail = json!({});
            if let (Some(object), Some(source)) = (detail.as_object_mut(), task.as_object()) {
                for (key, value) in source {
                    if key != "labels" && key != "externalLinks" && key != "assigneeImage" {
                        object.insert(key.clone(), value.clone());
                    }
                }
                object.insert("customFields".to_string(), json!([]));
            }
            Response::json(200, &detail)
        }
        None => not_found(),
    }
}

fn update_task(state: &mut DemoState, id: &str, request: &Request) -> Response {
    let body = request.json_body();
    let old_status = find_task_mut(&mut state.board, id)
        .and_then(|task| task["status"].as_str().map(str::to_string));
    let new_status = body
        .get("status")
        .and_then(Value::as_str)
        .map(str::to_string)
        .or(old_status.clone());

    {
        let Some(task) = find_task_mut(&mut state.board, id) else {
            return not_found();
        };
        if let Some(object) = task.as_object_mut() {
            for key in [
                "title",
                "description",
                "startDate",
                "dueDate",
                "priority",
                "position",
            ] {
                if let Some(value) = body.get(key) {
                    object.insert(key.to_string(), value.clone());
                }
            }
            if let Some(user) = body.get("userId") {
                object.insert("userId".to_string(), user.clone());
                let name = member_name(user);
                object.insert("assigneeName".to_string(), json!(name));
                object.insert("assigneeId".to_string(), user.clone());
                object.insert("assigneeImage".to_string(), Value::Null);
            }
        }
    }

    // A changed status has to move the task between column arrays, not just
    // rewrite the field — the board renders from the column lists.
    let status_changed = new_status
        .as_deref()
        .is_some_and(|status| old_status.as_deref().is_none_or(|old| old != status));
    if status_changed {
        let status = new_status.clone().unwrap_or_default();
        if apply_status_change(state, id, &status).is_none() {
            return not_found();
        }
    }

    single_task(state, id)
}

fn patch_task_field(state: &mut DemoState, id: &str, field: &str, request: &Request) -> Response {
    let body = request.json_body();
    let value = body.get(field).cloned().unwrap_or(Value::Null);

    if field == "status" {
        let destination = value.as_str().unwrap_or_default().to_string();
        if destination.is_empty() {
            return not_found();
        }
        let Some(task) = apply_status_change(state, id, &destination) else {
            return not_found();
        };
        return Response::json(200, &task);
    }

    let Some(task) = find_task_mut(&mut state.board, id) else {
        return not_found();
    };
    task[field] = value;
    if field == "userId" {
        assignee_fields(task);
    }
    single_task(state, id)
}

fn move_task(state: &mut DemoState, id: &str, request: &Request) -> Response {
    let body = request.json_body();
    let destination = body
        .get("destinationStatus")
        .or_else(|| body.get("status"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    if destination.is_empty() {
        return not_found();
    }

    match apply_status_change(state, id, &destination) {
        Some(task) => Response::json(200, &task),
        None => not_found(),
    }
}

fn create_task(state: &mut DemoState, project_id: &str, request: &Request) -> Response {
    if project_id != PROJECT_ID {
        return not_found();
    }
    let body = request.json_body();
    let title = body
        .get("title")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    if title.is_empty() {
        return Response::json(400, &json!({ "message": "A task needs a title." }));
    }
    let status = body
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("to-do")
        .to_string();
    let number = highest_task_number(state).unwrap_or(9) + 1;

    let mut task = json!({
        "id": format!("t-{}", uuid::Uuid::new_v4()),
        "title": title,
        "number": number,
        "description": body.get("description").cloned().unwrap_or(Value::Null),
        "status": status,
        "priority": body.get("priority").cloned().unwrap_or(json!("no-priority")),
        "startDate": body.get("startDate").cloned().unwrap_or(Value::Null),
        "dueDate": body.get("dueDate").cloned().unwrap_or(Value::Null),
        "position": next_position(&state.board, &status),
        "createdAt": now_iso(),
        "userId": body.get("userId").cloned().unwrap_or(Value::Null),
        "assigneeName": null,
        "assigneeId": null,
        "assigneeImage": null,
        "projectId": PROJECT_ID,
        "labels": [],
        "externalLinks": [],
    });
    assignee_fields(&mut task);

    if let Some(column) = column_mut(&mut state.board, &status) {
        if let Some(tasks) = column.get_mut("tasks").and_then(Value::as_array_mut) {
            tasks.push(task.clone());
        }
    }

    Response::json(200, &task)
}

fn delete_task(state: &mut DemoState, id: &str) -> Response {
    let Some(columns) = state
        .board
        .get_mut("data")
        .and_then(|data| data.get_mut("columns"))
        .and_then(Value::as_array_mut)
    else {
        return not_found();
    };
    for column in columns.iter_mut() {
        if let Some(tasks) = column.get_mut("tasks").and_then(Value::as_array_mut) {
            tasks.retain(|task| task["id"] != *id);
        }
    }
    Response::json(200, &json!({}))
}

fn bulk_tasks(state: &mut DemoState, request: &Request) -> Response {
    let body = request.json_body();
    let ids: Vec<String> = body
        .get("taskIds")
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    let operation = body
        .get("operation")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let value = body.get("value").cloned();

    for id in &ids {
        let Some(task) = find_task_mut(&mut state.board, id) else {
            continue;
        };
        match operation {
            "updateStatus" => {
                if let Some(value) = &value {
                    task["status"] = value.clone();
                }
            }
            "updatePriority" => {
                if let Some(value) = &value {
                    task["priority"] = value.clone();
                }
            }
            "updateAssignee" => {
                task["userId"] = value.clone().unwrap_or(Value::Null);
                assignee_fields(task);
            }
            "updateDueDate" => {
                task["dueDate"] = value.clone().unwrap_or(Value::Null);
            }
            _ => {}
        }
    }
    if operation == "delete" {
        for column in state
            .board
            .get_mut("data")
            .and_then(|data| data.get_mut("columns"))
            .and_then(Value::as_array_mut)
            .into_iter()
            .flatten()
        {
            if let Some(tasks) = column.get_mut("tasks").and_then(Value::as_array_mut) {
                tasks.retain(|task| {
                    !ids.contains(&task["id"].as_str().unwrap_or_default().to_string())
                });
            }
        }
    }
    Response::json(200, &json!({ "success": true, "updatedCount": ids.len() }))
}

fn column_mut<'a>(board: &'a mut Value, slug: &str) -> Option<&'a mut Value> {
    board
        .get_mut("data")?
        .get_mut("columns")?
        .as_array_mut()?
        .iter_mut()
        .find(|column| column["slug"] == *slug)
}

fn next_position(board: &Value, slug: &str) -> u64 {
    board["data"]["columns"]
        .as_array()
        .and_then(|columns| {
            columns
                .iter()
                .find(|column| column["slug"] == *slug)
                .and_then(|column| {
                    column["tasks"].as_array().map(|tasks| {
                        tasks
                            .iter()
                            .filter_map(|task| task["position"].as_u64())
                            .max()
                            .unwrap_or(0)
                            + 1000
                    })
                })
        })
        .unwrap_or(1000)
}

fn create_column(state: &mut DemoState, request: &Request) -> Response {
    let body = request.json_body();
    let name = body
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    if name.is_empty() {
        return Response::json(400, &json!({ "message": "A column needs a name." }));
    }
    let slug = name
        .to_lowercase()
        .replace(
            |character: char| !char::is_ascii_lowercase(&character) && character != '-',
            "-",
        )
        .trim_matches('-')
        .to_string();
    let column = json!({
        "id": slug,
        "slug": slug,
        "name": name,
        "icon": body.get("icon").cloned().unwrap_or(json!("circle")),
        "isFinal": false,
        "tasks": [],
    });
    if let Some(columns) = state.board["data"]
        .get_mut("columns")
        .and_then(Value::as_array_mut)
    {
        columns.push(column.clone());
    }
    Response::json(200, &column)
}

fn patch_column(state: &mut DemoState, id: &str, request: &Request) -> Response {
    let body = request.json_body();
    let columns = match state.board.get_mut("data") {
        Some(data) => data.get_mut("columns").and_then(Value::as_array_mut),
        None => None,
    };
    let Some(columns) = columns else {
        return not_found();
    };
    let Some(column) = columns.iter_mut().find(|column| column["id"] == *id) else {
        return not_found();
    };
    if let Some(name) = body.get("name").and_then(Value::as_str) {
        column["name"] = json!(name);
    }
    let updated = column.clone();
    Response::json(200, &updated)
}

fn delete_column(state: &mut DemoState, id: &str) -> Response {
    let Some(columns) = state
        .board
        .get_mut("data")
        .and_then(|data| data.get_mut("columns"))
        .and_then(Value::as_array_mut)
    else {
        return not_found();
    };

    let Some(position) = columns.iter().position(|column| column["id"] == *id) else {
        return not_found();
    };

    // Tasks in a deleted column fall back to the first column, so nothing the
    // demo user drags there can vanish.
    let column = columns.remove(position);
    let tasks = column["tasks"].as_array().cloned().unwrap_or_default();
    let first_slug = columns
        .first()
        .map(|column| column["slug"].as_str().unwrap_or("to-do").to_string())
        .unwrap_or_else(|| "to-do".to_string());
    for task in tasks {
        let mut task = task;
        task["status"] = json!(first_slug.clone());
        if let Some(target) = column_mut(&mut state.board, &first_slug) {
            if let Some(list) = target.get_mut("tasks").and_then(Value::as_array_mut) {
                list.push(task);
            }
        }
    }

    Response::json(200, &json!({}))
}

// --- Comments ---------------------------------------------------------------

fn comments_payload(state: &DemoState, task_id: &str) -> Response {
    let comments = state.comments.get(task_id).cloned().unwrap_or_default();
    Response::json(200, &Value::Array(comments))
}

fn create_comment(state: &mut DemoState, task_id: &str, request: &Request) -> Response {
    let body = request.json_body();
    let content = body
        .get("content")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    if content.is_empty() {
        return Response::json(400, &json!({ "message": "A comment needs content." }));
    }
    let comment = json!({
        "id": format!("cm-{}", uuid::Uuid::new_v4()),
        "taskId": task_id,
        "userId": DEMO_USER_ID,
        "content": content,
        "createdAt": now_iso(),
        "updatedAt": now_iso(),
        "user": { "name": "Demo User", "image": null }
    });
    state
        .comments
        .entry(task_id.to_string())
        .or_default()
        .push(comment.clone());
    Response::json(200, &comment)
}

fn update_comment(state: &mut DemoState, id: &str, request: &Request) -> Response {
    let content = request
        .json_body()
        .get("content")
        .cloned()
        .unwrap_or(Value::Null);
    for comments in state.comments.values_mut() {
        if let Some(comment) = comments.iter_mut().find(|comment| comment["id"] == *id) {
            comment["content"] = content;
            comment["updatedAt"] = json!(now_iso());
            return Response::json(200, comment);
        }
    }
    not_found()
}

fn delete_comment(state: &mut DemoState, id: &str) -> Response {
    for comments in state.comments.values_mut() {
        let before = comments.len();
        comments.retain(|comment| comment["id"] != *id);
        if comments.len() < before {
            return Response::json(200, &json!({}));
        }
    }
    not_found()
}

// --- Activity ---------------------------------------------------------------

fn activities_payload(state: &DemoState, task_id: &str) -> Response {
    let activities = state.activities.get(task_id).cloned().unwrap_or_default();
    Response::json(200, &Value::Array(activities))
}

fn create_activity(state: &mut DemoState, request: &Request) -> Response {
    let body = request.json_body();
    let task_id = body
        .get("taskId")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    if task_id.is_empty() {
        return Response::json(400, &json!({ "message": "An activity needs a task." }));
    }
    let activity = json!({
        "id": format!("act-{}", uuid::Uuid::new_v4()),
        "taskId": task_id,
        "type": body.get("type").cloned().unwrap_or(json!("task")),
        "content": body.get("message").or_else(|| body.get("content")).cloned().unwrap_or(Value::Null),
        "eventData": body.get("eventData").cloned().unwrap_or(Value::Null),
        "createdAt": now_iso(),
        "updatedAt": now_iso(),
        "userId": DEMO_USER_ID,
        "externalUserName": null, "externalUserAvatar": null, "externalSource": null, "externalUrl": null,
    });
    state
        .activities
        .entry(task_id)
        .or_default()
        .push(activity.clone());
    Response::json(200, &activity)
}

// --- Labels -----------------------------------------------------------------

fn task_labels_payload(state: &DemoState, task_id: &str) -> Response {
    let labels: Vec<Value> = state.board["data"]["columns"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|column| column["tasks"].as_array().cloned().unwrap_or_default())
        .filter(|task| task["id"].as_str() == Some(task_id))
        .flat_map(|task| task["labels"].as_array().cloned().unwrap_or_default())
        .map(|mut label| {
            label["taskId"] = json!(task_id);
            label
        })
        .collect();
    Response::json(200, &Value::Array(labels))
}

fn create_label(state: &mut DemoState, request: &Request) -> Response {
    let body = request.json_body();
    let name = body
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    if name.is_empty() {
        return Response::json(400, &json!({ "message": "A label needs a name." }));
    }
    let label = json!({
        "id": format!("lbl-{}", uuid::Uuid::new_v4()),
        "name": name,
        "color": body.get("color").cloned().unwrap_or(json!("#64748b")),
        "createdAt": now_iso(),
        "updatedAt": now_iso(),
        "taskId": body.get("taskId").cloned().unwrap_or(Value::Null),
        "workspaceId": body.get("workspaceId").cloned().unwrap_or(json!(WORKSPACE_ID)),
    });
    state.labels.push(label.clone());
    Response::json(200, &label)
}

fn patch_label(state: &mut DemoState, id: &str, request: &Request) -> Response {
    let body = request.json_body();
    let Some(label) = state.labels.iter_mut().find(|label| label["id"] == *id) else {
        return not_found();
    };
    if let Some(name) = body.get("name") {
        label["name"] = name.clone();
    }
    if let Some(color) = body.get("color") {
        label["color"] = color.clone();
    }
    Response::json(200, label)
}

fn delete_label(state: &mut DemoState, id: &str) -> Response {
    state.labels.retain(|label| label["id"] != *id);
    for column in state
        .board
        .get_mut("data")
        .and_then(|data| data.get_mut("columns"))
        .and_then(Value::as_array_mut)
        .into_iter()
        .flatten()
    {
        for task in column
            .get_mut("tasks")
            .and_then(Value::as_array_mut)
            .into_iter()
            .flatten()
        {
            if let Some(task_labels) = task.get_mut("labels").and_then(Value::as_array_mut) {
                task_labels.retain(|label| label["id"] != *id);
            }
        }
    }
    Response::json(200, &json!({}))
}

fn attach_label(state: &mut DemoState, id: &str, request: &Request) -> Response {
    let body = request.json_body();
    let task_id = body
        .get("taskId")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let Some(label) = state.labels.iter().find(|label| label["id"] == *id) else {
        return not_found();
    };
    let Some(task) = find_task_mut(&mut state.board, &task_id) else {
        return not_found();
    };
    if let Some(task_labels) = task.get_mut("labels").and_then(Value::as_array_mut) {
        if !task_labels.iter().any(|attached| attached["id"] == *id) {
            task_labels.push(json!({ "id": id, "name": label["name"], "color": label["color"] }));
        }
    }
    Response::json(200, &json!({}))
}

fn detach_label(state: &mut DemoState, id: &str, request: &Request) -> Response {
    let body = request.json_body();
    let task_id = body
        .get("taskId")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let Some(task) = find_task_mut(&mut state.board, &task_id) else {
        return not_found();
    };
    if let Some(task_labels) = task.get_mut("labels").and_then(Value::as_array_mut) {
        task_labels.retain(|attached| attached["id"] != *id);
    }
    Response::json(200, &json!({}))
}

// --- Time entries -----------------------------------------------------------

fn time_entries_payload(state: &DemoState, task_id: &str) -> Response {
    let entries = state.time_entries.get(task_id).cloned().unwrap_or_default();
    Response::json(200, &Value::Array(entries))
}

fn create_time_entry(state: &mut DemoState, request: &Request) -> Response {
    let body = request.json_body();
    let task_id = body
        .get("taskId")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    if task_id.is_empty() {
        return Response::json(400, &json!({ "message": "A time entry needs a task." }));
    }
    let entry = json!({
        "id": format!("te-{}", uuid::Uuid::new_v4()),
        "taskId": task_id,
        "userId": DEMO_USER_ID,
        "description": body.get("description").cloned().unwrap_or(Value::Null),
        "startTime": body.get("startTime").cloned().unwrap_or(json!(now_iso())),
        "endTime": body.get("endTime").cloned().unwrap_or(Value::Null),
        "duration": body.get("duration").cloned().unwrap_or(Value::Null),
        "createdAt": now_iso(),
        "updatedAt": now_iso(),
        "userName": "Demo User"
    });
    state
        .time_entries
        .entry(task_id)
        .or_default()
        .push(entry.clone());
    Response::json(200, &entry)
}

// --- Project mutations ------------------------------------------------------

fn create_project(_state: &mut DemoState, request: &Request) -> Response {
    let body = request.json_body();
    let name = body
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    if name.is_empty() {
        return Response::json(400, &json!({ "message": "A project needs a name." }));
    }
    let project = json!({
        "id": format!("{}", uuid::Uuid::new_v4()),
        "workspaceId": body.get("workspaceId").cloned().unwrap_or(json!(WORKSPACE_ID)),
        "slug": body.get("slug").cloned().unwrap_or(json!("NEW")),
        "icon": body.get("icon").cloned().unwrap_or(json!("Layout")),
        "name": name,
        "description": body.get("description").cloned().unwrap_or(Value::Null),
        "createdAt": now_iso(),
        "isPublic": false,
        "archivedAt": null,
        "position": 2000,
        "lastTaskNumber": 0,
        "statistics": { "completionPercentage": 0, "totalTasks": 0, "dueDate": null },
        "archivedTasks": [], "plannedTasks": [], "columns": []
    });
    Response::json(200, &project)
}

fn update_project(state: &mut DemoState, id: &str, request: &Request) -> Response {
    let _ = (state, id, request);
    Response::json(200, &json!({}))
}

fn delete_project(state: &mut DemoState, id: &str) -> Response {
    let _ = state;
    if id == PROJECT_ID {
        // The demo's only project stays: deleting it would leave an empty app.
        return forbidden();
    }
    Response::json(200, &json!({}))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(method: &str, path: &str, cookie: Option<&str>, body: Value) -> Request {
        let mut headers = Vec::new();
        if let Some(cookie) = cookie {
            headers.push(("Cookie".to_string(), cookie.to_string()));
        }
        Request {
            method: method.to_uppercase(),
            path: path.to_string(),
            headers,
            body: serde_json::to_vec(&body).unwrap_or_default(),
        }
    }

    fn runtime_with_fresh_state() -> DemoRuntime {
        DemoRuntime {
            state: Mutex::new(build_state().unwrap()),
            payload: Mutex::new(ZipArchive::new(File::open(payload_for_tests()).unwrap()).unwrap()),
        }
    }

    fn payload_for_tests() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/web-demo.zip")
    }

    #[test]
    fn session_gates_the_protected_api() {
        let runtime = runtime_with_fresh_state();

        let session = route(
            &runtime,
            &request("GET", "/api/auth/get-session", None, json!({})),
        );
        assert_eq!(session.status, 200);

        let tasks = route(
            &runtime,
            &request("GET", "/api/task/tasks/other", None, json!({})),
        );
        assert_eq!(tasks.status, 401, "protected reads need the session cookie");
    }

    #[test]
    fn sign_in_flow_issues_a_session() {
        let runtime = runtime_with_fresh_state();

        let wrong = route(
            &runtime,
            &request(
                "POST",
                "/api/auth/sign-in/email",
                None,
                json!({ "email": DEMO_EMAIL, "password": "wrong" }),
            ),
        );
        assert_eq!(wrong.status, 401);

        let good = route(
            &runtime,
            &request(
                "POST",
                "/api/auth/sign-in/email",
                None,
                json!({ "email": DEMO_EMAIL, "password": DEMO_PASSWORD }),
            ),
        );
        assert_eq!(good.status, 200);
        let cookie_ok = good.headers.iter().any(|(name, value)| {
            name == "Set-Cookie" && value.contains(SESSION_COOKIE_TOKEN) && value.contains("Path=/")
        });
        assert!(
            cookie_ok,
            "the session cookie must not be scoped to the auth path"
        );

        let session = route(
            &runtime,
            &request(
                "GET",
                "/api/auth/get-session",
                Some(SESSION_COOKIE),
                json!({}),
            ),
        );
        assert_eq!(session.status, 200);
        let body: Value = serde_json::from_slice(&session.body).unwrap();
        assert_eq!(
            body["session"]["activeOrganizationId"], WORKSPACE_ID,
            "the demo session points at the example workspace"
        );
    }

    #[test]
    fn board_serves_the_example_workspace_shape() {
        let runtime = runtime_with_fresh_state();

        let board = route(
            &runtime,
            &request(
                "GET",
                &format!("/api/task/tasks/{PROJECT_ID}"),
                Some(SESSION_COOKIE),
                json!({}),
            ),
        );
        assert_eq!(board.status, 200);
        let body: Value = serde_json::from_slice(&board.body).unwrap();
        let columns = body["data"]["columns"].as_array().unwrap();
        assert_eq!(columns.len(), 4, "the board ships four columns");
        assert_eq!(body["data"]["id"], PROJECT_ID);
        assert!(
            columns
                .iter()
                .any(|column| column["slug"] == "done" && column["isFinal"] == true),
            "the done column must be final"
        );
    }

    #[test]
    fn moving_a_task_moves_it_in_memory() {
        let runtime = runtime_with_fresh_state();

        let before: Value = serde_json::from_slice(
            &route(
                &runtime,
                &request(
                    "GET",
                    &format!("/api/task/tasks/{PROJECT_ID}"),
                    Some(SESSION_COOKIE),
                    json!({}),
                ),
            )
            .body,
        )
        .unwrap();
        let task_id = before["data"]["columns"][0]["tasks"][0]["id"]
            .as_str()
            .unwrap()
            .to_string();

        let moved = route(
            &runtime,
            &request(
                "POST",
                &format!("/api/task/move/{task_id}"),
                Some(SESSION_COOKIE),
                json!({ "destinationStatus": "done" }),
            ),
        );
        assert_eq!(moved.status, 200);
        let body: Value = serde_json::from_slice(&moved.body).unwrap();
        assert_eq!(body["status"], "done");

        let after: Value = serde_json::from_slice(
            &route(
                &runtime,
                &request(
                    "GET",
                    &format!("/api/task/tasks/{PROJECT_ID}"),
                    Some(SESSION_COOKIE),
                    json!({}),
                ),
            )
            .body,
        )
        .unwrap();
        let now_in_done = after["data"]["columns"]
            .as_array()
            .unwrap()
            .iter()
            .find(|column| column["slug"] == "done")
            .unwrap()["tasks"]
            .as_array()
            .unwrap()
            .iter()
            .any(|task| task["id"] == task_id.as_str());
        assert!(now_in_done, "the moved task should be in the done column");
    }

    /// The board's drop handler replays every affected task through
    /// PUT /api/task/{id} with a full body (empty userId allowed). The board
    /// refetches after it, so the PUT has to actually land in the state.
    #[test]
    fn put_update_moves_a_task_between_columns() {
        let runtime = runtime_with_fresh_state();

        let body = json!({
            "userId": "",
            "title": "Set up web analytics and error tracking",
            "description": "",
            "status": "in-progress",
            "priority": "no-priority",
            "startDate": null,
            "dueDate": null,
            "position": 2000,
            "projectId": PROJECT_ID,
        });
        let updated = route(
            &runtime,
            &request("PUT", "/api/task/t6-analytics", Some(SESSION_COOKIE), body),
        );
        assert_eq!(updated.status, 200, "the PUT must be accepted");
        let updated_body: Value = serde_json::from_slice(&updated.body).unwrap();
        println!("PUT response: {updated_body}");

        let board: Value = serde_json::from_slice(
            &route(
                &runtime,
                &request(
                    "GET",
                    &format!("/api/task/tasks/{PROJECT_ID}"),
                    Some(SESSION_COOKIE),
                    json!({}),
                ),
            )
            .body,
        )
        .unwrap();
        let placed: Vec<&str> = board["data"]["columns"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|column| {
                column["tasks"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|task| task["id"] == "t6-analytics")
                    .map(|_task| column["slug"].as_str().unwrap())
            })
            .collect();
        assert_eq!(placed, ["in-progress"], "the PUT must move the task");
    }

    #[test]
    fn comments_survive_across_requests() {
        let runtime = runtime_with_fresh_state();

        let created = route(
            &runtime,
            &request(
                "POST",
                "/api/comment/t8-palette",
                Some(SESSION_COOKIE),
                json!({ "content": "Looks good to me." }),
            ),
        );
        assert_eq!(created.status, 200);

        let listed = route(
            &runtime,
            &request(
                "GET",
                "/api/comment/t8-palette",
                Some(SESSION_COOKIE),
                json!({}),
            ),
        );
        let body: Value = serde_json::from_slice(&listed.body).unwrap();
        assert!(
            body.as_array()
                .unwrap()
                .iter()
                .any(|comment| comment["content"] == "Looks good to me."),
            "the posted comment should be served back"
        );
    }

    #[test]
    fn static_files_come_from_the_payload() {
        let runtime = runtime_with_fresh_state();

        let index = route(&runtime, &request("GET", "/", None, json!({})));
        assert_eq!(index.status, 200);
        let html = String::from_utf8_lossy(&index.body);
        assert!(html.contains("<!doctype html") || html.contains("<!doctype html"));
        assert!(
            html.contains("kaneo.demo.desktop") || html.contains("prefill") || html.len() > 1000,
            "the shell page should be served"
        );

        let unknown_spa_route = route(
            &runtime,
            &request("GET", "/dashboard/workspace/x", None, json!({})),
        );
        assert_eq!(
            unknown_spa_route.status, 200,
            "SPA routes fall back to index.html"
        );

        let asset = route(
            &runtime,
            &request("GET", "/assets/index-B28_2S2k.js", None, json!({})),
        );
        assert_eq!(asset.status, 200);
        assert_eq!(
            asset
                .headers
                .iter()
                .find(|(name, _)| name == "Content-Type")
                .map(|(_, value)| value.as_str()),
            Some("text/javascript; charset=utf-8")
        );
    }

    /// One real socket round-trip: the same path a demo window's webview
    /// drives, from read_request through write_response, including the
    /// websocket upgrade handshake.
    #[test]
    fn the_socket_loop_serves_requests_and_upgrades() {
        let runtime = runtime_with_fresh_state();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();

        std::thread::spawn(move || {
            if let Ok((stream, _)) = listener.accept() {
                let _ = handle_connection(&runtime, stream);
            }
        });

        let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
        stream
            .write_all(
                format!(
                    "GET /api/auth/get-session HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n"
                )
                .as_bytes(),
            )
            .unwrap();
        let mut response = String::new();
        let _ = stream.read_to_string(&mut response);
        assert!(response.starts_with("HTTP/1.1 200"), "got: {response}");

        // The SPA's live-update socket: an upgrade is answered with 101 and
        // the connection is held open rather than errored.
        let runtime2 = runtime_with_fresh_state();
        let listener2 = TcpListener::bind("127.0.0.1:0").unwrap();
        let port2 = listener2.local_addr().unwrap().port();
        std::thread::spawn(move || {
            if let Ok((stream, _)) = listener2.accept() {
                let _ = handle_connection(&runtime2, stream);
            }
        });
        let mut stream2 = TcpStream::connect(("127.0.0.1", port2)).unwrap();
        stream2
            .write_all(
                format!(
                    "GET /api/ws/user?windowId=abc HTTP/1.1\r\nHost: 127.0.0.1:{port2}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\r\n"
                )
                .as_bytes(),
            )
            .unwrap();
        // The upgrade handshake arrives immediately; the server then parks the
        // socket, so read once and drop instead of reading to EOF.
        let mut upgraded = [0u8; 256];
        let read = stream2.read(&mut upgraded).unwrap();
        let upgraded = String::from_utf8_lossy(&upgraded[..read]).into_owned();
        assert!(upgraded.starts_with("HTTP/1.1 101"), "got: {upgraded}");
        assert!(
            upgraded.contains("Sec-WebSocket-Accept: s3pPLMBiTxaQ9kYGzzhZRbK+xOo="),
            "the accept key must follow the RFC 6455 sample: {upgraded}"
        );
        drop(stream2);
    }
}
