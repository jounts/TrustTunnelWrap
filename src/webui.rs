use crate::auth;
use crate::config::{TunnelSettings, WrapperConfig};
use crate::logs;
use crate::tunnel::TunnelManager;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};
use tiny_http::{Header, Method, Request, Response, Server, StatusCode};

const SESSION_TTL_SECS: u64 = 3600;
const INDEX_HTML: &str = include_str!("../package/www/index.html");

struct Sessions {
    tokens: HashMap<String, SystemTime>,
}

impl Sessions {
    fn new() -> Self {
        Self {
            tokens: HashMap::new(),
        }
    }

    fn create(&mut self) -> String {
        self.cleanup();
        let token = uuid::Uuid::new_v4().to_string();
        let expiry = SystemTime::now() + Duration::from_secs(SESSION_TTL_SECS);
        self.tokens.insert(token.clone(), expiry);
        token
    }

    fn validate(&mut self, token: &str) -> bool {
        if let Some(expiry) = self.tokens.get_mut(token) {
            if SystemTime::now() < *expiry {
                *expiry = SystemTime::now() + Duration::from_secs(SESSION_TTL_SECS);
                return true;
            }
            self.tokens.remove(token);
        }
        false
    }

    fn cleanup(&mut self) {
        let now = SystemTime::now();
        self.tokens.retain(|_, exp| now < *exp);
    }
}

pub struct WebUI {
    tunnel: Arc<TunnelManager>,
    config: Arc<Mutex<WrapperConfig>>,
    config_path: String,
    sessions: Mutex<Sessions>,
    ndm_host: String,
    ndm_port: u16,
}

impl WebUI {
    pub fn new(
        tunnel: Arc<TunnelManager>,
        config: Arc<Mutex<WrapperConfig>>,
        config_path: String,
        ndm_host: String,
        ndm_port: u16,
    ) -> Arc<Self> {
        Arc::new(Self {
            tunnel,
            config,
            config_path,
            sessions: Mutex::new(Sessions::new()),
            ndm_host,
            ndm_port,
        })
    }

    pub fn run(self: &Arc<Self>, bind: &str, port: u16) {
        let addr = format!("{}:{}", bind, port);
        let server = match Server::http(&addr) {
            Ok(s) => s,
            Err(e) => {
                log::error!("Failed to start WebUI on {}: {}", addr, e);
                return;
            }
        };

        log::info!("WebUI listening on http://{}", addr);
        logs::global_buffer().push(format!("[webui] listening on http://{}", addr));

        for request in server.incoming_requests() {
            let resp = self.handle_request(request);
            // Response is sent inside handle_request via request.respond()
            let _ = resp;
        }
    }

    fn handle_request(&self, mut request: Request) -> Result<(), ()> {
        let path = request.url().split('?').next().unwrap_or("/").to_string();
        let method = request.method().clone();

        let result = match (method, path.as_str()) {
            (Method::Get, "/") | (Method::Get, "/index.html") => self.serve_index(&request),
            (Method::Post, "/api/login") => self.api_login(&mut request),
            (Method::Get, "/api/status") => self.api_authed(&request, |s| s.api_status()),
            (Method::Get, "/api/config") => self.api_authed(&request, |s| s.api_get_config()),
            (Method::Post, "/api/config") => {
                let body = read_body(&mut request);
                self.api_authed(&request, |s| s.api_set_config(&body))
            }
            (Method::Post, "/api/control") => {
                let body = read_body(&mut request);
                self.api_authed(&request, |s| s.api_control(&body))
            }
            (Method::Get, "/api/logs") => self.api_authed(&request, |s| s.api_logs(&request)),
            _ => json_response(404, r#"{"error":"not found"}"#),
        };

        request.respond(result).map_err(|_| ())
    }

    fn serve_index(&self, _req: &Request) -> Response<std::io::Cursor<Vec<u8>>> {
        let data = INDEX_HTML.as_bytes().to_vec();
        Response::from_data(data)
            .with_header(
                Header::from_bytes("Content-Type", "text/html; charset=utf-8").unwrap(),
            )
            .with_header(Header::from_bytes("Cache-Control", "public, max-age=300").unwrap())
    }

    fn api_login(&self, request: &mut Request) -> Response<std::io::Cursor<Vec<u8>>> {
        let body = read_body(request);
        let parsed: Result<serde_json::Value, _> = serde_json::from_str(&body);
        let (login, password) = match parsed {
            Ok(v) => {
                let l = v.get("login").and_then(|v| v.as_str()).unwrap_or("");
                let p = v.get("password").and_then(|v| v.as_str()).unwrap_or("");
                (l.to_string(), p.to_string())
            }
            Err(_) => return json_response(400, r#"{"error":"invalid json"}"#),
        };

        if login.is_empty() || password.is_empty() {
            return json_response(400, r#"{"error":"login and password required"}"#);
        }

        if auth::authenticate(&self.ndm_host, self.ndm_port, &login, &password) {
            let token = self.sessions.lock().unwrap().create();
            log::info!("WebUI: user '{}' logged in", login);
            json_response(
                200,
                &serde_json::json!({"token": token, "status": "ok"}).to_string(),
            )
        } else {
            log::warn!("WebUI: failed login attempt for '{}'", login);
            json_response(401, r#"{"error":"invalid credentials"}"#)
        }
    }

    fn api_authed<F>(&self, request: &Request, handler: F) -> Response<std::io::Cursor<Vec<u8>>>
    where
        F: FnOnce(&Self) -> Response<std::io::Cursor<Vec<u8>>>,
    {
        let token = get_auth_header(request);
        let valid = match token {
            Some(t) => self.sessions.lock().unwrap().validate(&t),
            None => false,
        };

        if !valid {
            return json_response(401, r#"{"error":"unauthorized"}"#);
        }

        handler(self)
    }

    fn api_status(&self) -> Response<std::io::Cursor<Vec<u8>>> {
        let st = self.tunnel.get_status();
        let body = serde_json::json!({
            "connected": st.connected,
            "uptime_seconds": st.uptime_seconds,
            "last_error": st.last_error,
            "pid": st.pid,
        });
        json_response(200, &body.to_string())
    }

    fn api_get_config(&self) -> Response<std::io::Cursor<Vec<u8>>> {
        let cfg = self.config.lock().unwrap();
        let body = serde_json::to_string(&cfg.tunnel).unwrap_or_default();
        json_response(200, &body)
    }

    fn api_set_config(&self, body: &str) -> Response<std::io::Cursor<Vec<u8>>> {
        let new_tunnel: TunnelSettings = match serde_json::from_str(body) {
            Ok(v) => v,
            Err(e) => {
                return json_response(
                    400,
                    &serde_json::json!({"error": format!("invalid config: {}", e)}).to_string(),
                )
            }
        };

        // Update in-memory config
        {
            let mut cfg = self.config.lock().unwrap();
            cfg.tunnel = new_tunnel.clone();
            if let Err(e) = cfg.save(&self.config_path) {
                log::error!("Failed to save config: {}", e);
            }
        }

        // Update tunnel settings (will take effect on next restart)
        self.tunnel.update_settings(new_tunnel);

        json_response(200, r#"{"status":"updated"}"#)
    }

    fn api_control(&self, body: &str) -> Response<std::io::Cursor<Vec<u8>>> {
        let parsed: serde_json::Value = match serde_json::from_str(body) {
            Ok(v) => v,
            Err(_) => return json_response(400, r#"{"error":"invalid json"}"#),
        };

        let action = parsed
            .get("action")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        match action {
            "connect" => match self.tunnel.start() {
                Ok(()) => json_response(200, r#"{"status":"connecting"}"#),
                Err(e) => json_response(
                    400,
                    &serde_json::json!({"error": e}).to_string(),
                ),
            },
            "disconnect" => {
                self.tunnel.stop();
                json_response(200, r#"{"status":"disconnected"}"#)
            }
            "restart" => match self.tunnel.restart() {
                Ok(()) => json_response(200, r#"{"status":"restarting"}"#),
                Err(e) => json_response(
                    400,
                    &serde_json::json!({"error": e}).to_string(),
                ),
            },
            _ => json_response(400, r#"{"error":"unknown action"}"#),
        }
    }

    fn api_logs(&self, request: &Request) -> Response<std::io::Cursor<Vec<u8>>> {
        let limit = parse_query_param(request.url(), "limit")
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(100)
            .min(500);

        let lines = logs::get_combined_logs(limit);
        let body = serde_json::json!({
            "lines": lines,
            "total": logs::global_buffer().len(),
        });
        json_response(200, &body.to_string())
    }
}

fn json_response(status: u16, body: &str) -> Response<std::io::Cursor<Vec<u8>>> {
    let data = body.as_bytes().to_vec();
    Response::from_data(data)
        .with_status_code(StatusCode(status))
        .with_header(Header::from_bytes("Content-Type", "application/json").unwrap())
        .with_header(
            Header::from_bytes("Access-Control-Allow-Origin", "*").unwrap(),
        )
}

fn read_body(request: &mut Request) -> String {
    let mut body = String::new();
    let _ = request.as_reader().read_to_string(&mut body);
    body
}

fn get_auth_header(request: &Request) -> Option<String> {
    for header in request.headers() {
        let name = header.field.as_str().as_str();
        if name.eq_ignore_ascii_case("authorization") {
            return Some(header.value.as_str().to_string());
        }
    }
    None
}

fn parse_query_param(url: &str, key: &str) -> Option<String> {
    let query = url.split('?').nth(1)?;
    for pair in query.split('&') {
        let mut kv = pair.splitn(2, '=');
        if let (Some(k), Some(v)) = (kv.next(), kv.next()) {
            if k == key {
                return Some(v.to_string());
            }
        }
    }
    None
}
