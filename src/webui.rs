use crate::auth;
use crate::config::{SplitTunnelSettings, TunnelSettings, WrapperConfig};
use crate::logs;
use crate::split_tunnel::SplitTunnelManager;
use crate::tunnel::TunnelManager;
use std::collections::HashMap;
use std::io::Read;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};
use tiny_http::{Header, Method, Request, Response, Server, StatusCode};

const SESSION_TTL_SECS: u64 = 3600;
const MAX_BODY_BYTES: usize = 64 * 1024;
const INDEX_HTML: &str = include_str!("../package/www/index.html");
const WEBUI_VERSION: &str = env!("TRUSTTUNNEL_VERSION");

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
    split: Arc<SplitTunnelManager>,
}

impl WebUI {
    pub fn new(
        tunnel: Arc<TunnelManager>,
        config: Arc<Mutex<WrapperConfig>>,
        config_path: String,
        ndm_host: String,
        ndm_port: u16,
        split: Arc<SplitTunnelManager>,
    ) -> Arc<Self> {
        Arc::new(Self {
            tunnel,
            config,
            config_path,
            sessions: Mutex::new(Sessions::new()),
            ndm_host,
            ndm_port,
            split,
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
        log::warn!(
            "WebUI is using unauthenticated transport (HTTP) on {}; session tokens can be intercepted",
            addr
        );
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
            (Method::Post, "/api/config") => match read_body(&mut request) {
                Ok(body) => self.api_authed(&request, |s| s.api_set_config(&body)),
                Err(e) => json_response(413, &serde_json::json!({"error": e}).to_string()),
            },
            (Method::Post, "/api/control") => match read_body(&mut request) {
                Ok(body) => self.api_authed(&request, |s| s.api_control(&body)),
                Err(e) => json_response(413, &serde_json::json!({"error": e}).to_string()),
            },
            (Method::Get, "/api/logs") => self.api_authed(&request, |s| s.api_logs(&request)),
            (Method::Get, "/api/geoip/status") => {
                self.api_authed(&request, |s| s.api_geoip_status())
            }
            (Method::Get, "/api/geoip/providers") => {
                self.api_authed(&request, |s| s.api_geoip_providers())
            }
            (Method::Post, "/api/geoip/provider") => match read_body(&mut request) {
                Ok(body) => self.api_authed(&request, move |s| s.api_geoip_select_provider(&body)),
                Err(e) => json_response(413, &serde_json::json!({"error": e}).to_string()),
            },
            (Method::Post, "/api/geoip/update") => match read_body(&mut request) {
                Ok(body) => self.api_authed(&request, move |s| s.api_geoip_update(&body)),
                Err(e) => json_response(413, &serde_json::json!({"error": e}).to_string()),
            },
            (Method::Get, "/api/splittunnel/policy") => {
                self.api_authed(&request, |s| s.api_split_get_policy())
            }
            (Method::Post, "/api/splittunnel/policy") => match read_body(&mut request) {
                Ok(body) => self.api_authed(&request, move |s| s.api_split_set_policy(&body)),
                Err(e) => json_response(413, &serde_json::json!({"error": e}).to_string()),
            },
            (Method::Post, "/api/splittunnel/test") => match read_body(&mut request) {
                Ok(body) => self.api_authed(&request, move |s| s.api_split_test(&body)),
                Err(e) => json_response(413, &serde_json::json!({"error": e}).to_string()),
            },
            _ => json_response(404, r#"{"error":"not found"}"#),
        };

        request.respond(result).map_err(|_| ())
    }

    fn serve_index(&self, _req: &Request) -> Response<std::io::Cursor<Vec<u8>>> {
        let html = INDEX_HTML.replace("__TRUSTTUNNEL_VERSION__", WEBUI_VERSION);
        let data = html.into_bytes();
        Response::from_data(data)
            .with_header(Header::from_bytes("Content-Type", "text/html; charset=utf-8").unwrap())
            .with_header(Header::from_bytes("Cache-Control", "public, max-age=300").unwrap())
    }

    fn api_login(&self, request: &mut Request) -> Response<std::io::Cursor<Vec<u8>>> {
        let body = match read_body(request) {
            Ok(body) => body,
            Err(e) => return json_response(413, &serde_json::json!({"error": e}).to_string()),
        };
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
        if let Err(e) = new_tunnel.validate() {
            return json_response(
                400,
                &serde_json::json!({"error": format!("invalid config: {}", e)}).to_string(),
            );
        }

        // Save first, then swap in-memory state to keep API semantics transactional.
        {
            let mut cfg = self.config.lock().unwrap();
            let mut next_cfg = cfg.clone();
            next_cfg.tunnel = new_tunnel.clone();
            if let Err(e) = next_cfg.save(&self.config_path) {
                log::error!("Failed to save config: {}", e);
                return json_response(
                    500,
                    &serde_json::json!({"error": format!("failed to save config: {}", e)})
                        .to_string(),
                );
            }
            *cfg = next_cfg;
        }

        // Update tunnel settings (will take effect on next restart)
        let has_ipv6 = new_tunnel.has_ipv6;
        self.tunnel.update_settings(new_tunnel);
        {
            let geoip_cfg = { self.config.lock().unwrap().geoip.clone() };
            self.split
                .update_configs(self.split.split_settings(), geoip_cfg, has_ipv6);
        }

        json_response(200, r#"{"status":"updated"}"#)
    }

    fn api_control(&self, body: &str) -> Response<std::io::Cursor<Vec<u8>>> {
        let parsed: serde_json::Value = match serde_json::from_str(body) {
            Ok(v) => v,
            Err(_) => return json_response(400, r#"{"error":"invalid json"}"#),
        };

        let action = parsed.get("action").and_then(|v| v.as_str()).unwrap_or("");

        match action {
            "connect" => match self.tunnel.start() {
                Ok(()) => json_response(200, r#"{"status":"connecting"}"#),
                Err(e) => json_response(400, &serde_json::json!({"error": e}).to_string()),
            },
            "disconnect" => {
                self.tunnel.stop();
                json_response(200, r#"{"status":"disconnected"}"#)
            }
            "restart" => match self.tunnel.restart() {
                Ok(()) => json_response(200, r#"{"status":"restarting"}"#),
                Err(e) => json_response(400, &serde_json::json!({"error": e}).to_string()),
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

    fn api_geoip_status(&self) -> Response<std::io::Cursor<Vec<u8>>> {
        let geoip_cfg = self.config.lock().unwrap().geoip.clone();
        let mut body = self.split.status_json();
        if let Some(obj) = body.as_object_mut() {
            obj.insert(
                "config_enabled".into(),
                serde_json::json!(geoip_cfg.enabled),
            );
            obj.insert(
                "auto_update".into(),
                serde_json::json!({
                    "enabled": geoip_cfg.auto_update.enabled,
                    "interval_hours": geoip_cfg.auto_update.interval_hours,
                    "max_age_hours_hard": geoip_cfg.auto_update.max_age_hours_hard,
                }),
            );
        }
        json_response(200, &body.to_string())
    }

    fn api_geoip_providers(&self) -> Response<std::io::Cursor<Vec<u8>>> {
        let cfg = self.config.lock().unwrap();
        let body = serde_json::json!({
            "db_providers": cfg.geoip.db_providers,
            "api_providers": cfg.geoip.api_providers,
        });
        json_response(200, &body.to_string())
    }

    /// Triggers a database rebuild. Body may carry {"provider_id": "..."} —
    /// reserved for per-provider updates; the current build refreshes all.
    fn api_geoip_update(&self, body: &str) -> Response<std::io::Cursor<Vec<u8>>> {
        let _provider_id: Option<String> = serde_json::from_str::<serde_json::Value>(body)
            .ok()
            .and_then(|v| {
                v.get("provider_id")
                    .and_then(|p| p.as_str())
                    .map(String::from)
            });

        let (geoip_cfg, st_cfg) = {
            let cfg = self.config.lock().unwrap();
            (cfg.geoip.clone(), cfg.split_tunnel.clone())
        };
        match crate::geoip::rebuild_for_split_tunnel(&geoip_cfg, &st_cfg) {
            Ok(reports) => {
                self.split.reload_geoip();
                // Re-apply the policy with fresh data when active.
                if self.split.is_active() {
                    if let Err(e) = self.split.apply_now() {
                        log::warn!("[split] re-apply after update failed: {}", e);
                    }
                }
                json_response(
                    200,
                    &serde_json::json!({"status":"updated", "reports": reports}).to_string(),
                )
            }
            Err(e) => json_response(500, &serde_json::json!({"error": e}).to_string()),
        }
    }

    /// Selects the active db provider (single-select) and immediately
    /// downloads/builds its database. Selection = fetch, no extra click.
    fn api_geoip_select_provider(&self, body: &str) -> Response<std::io::Cursor<Vec<u8>>> {
        let wanted_id = match serde_json::from_str::<serde_json::Value>(body)
            .ok()
            .and_then(|v| v.get("id").and_then(|i| i.as_str()).map(String::from))
        {
            Some(id) => id,
            None => return json_response(400, r#"{"error":"missing 'id' field"}"#),
        };

        let mut geoip_cfg = self.config.lock().unwrap().geoip.clone();
        if let Err(e) = geoip_cfg.select_single_db_provider(&wanted_id) {
            return json_response(400, &serde_json::json!({"error": e}).to_string());
        }

        let st_cfg = { self.config.lock().unwrap().split_tunnel.clone() };
        match crate::geoip::rebuild_for_split_tunnel(&geoip_cfg, &st_cfg) {
            Ok(reports) => {
                // Persist only after a successful build.
                {
                    let mut cfg = self.config.lock().unwrap();
                    let mut next_cfg = cfg.clone();
                    next_cfg.geoip = geoip_cfg.clone();
                    if let Err(e) = next_cfg.save(&self.config_path) {
                        return json_response(
                            500,
                            &serde_json::json!({"error": format!("failed to save config: {}", e)})
                                .to_string(),
                        );
                    }
                    *cfg = next_cfg;
                }
                self.split.update_configs(
                    st_cfg.clone(),
                    geoip_cfg.clone(),
                    self.config.lock().unwrap().tunnel.has_ipv6,
                );
                if self.split.is_active() {
                    if let Err(e) = self.split.apply_now() {
                        log::warn!("[split] re-apply after provider switch failed: {}", e);
                    }
                }
                let meta = crate::geoip::db::DbMeta::load(std::path::Path::new(
                    geoip_cfg.db_path.trim_end_matches('/'),
                ));
                json_response(
                    200,
                    &serde_json::json!({"status":"updated", "provider": wanted_id, "reports": reports, "meta": meta})
                        .to_string(),
                )
            }
            Err(e) => json_response(
                500,
                &serde_json::json!({"error": format!("provider '{}' failed: {}", wanted_id, e)})
                    .to_string(),
            ),
        }
    }

    fn api_split_get_policy(&self) -> Response<std::io::Cursor<Vec<u8>>> {
        let cfg = self.config.lock().unwrap();
        let body = serde_json::json!({
            "policy": cfg.split_tunnel.policy,
            "countries_bypass": cfg.split_tunnel.countries_bypass,
            "countries_tunnel": cfg.split_tunnel.countries_tunnel,
            "manual_bypass": cfg.split_tunnel.manual_bypass,
            "manual_tunnel": cfg.split_tunnel.manual_tunnel,
            "detection_mode": cfg.split_tunnel.detection_mode,
            "enabled": cfg.split_tunnel.enabled,
            "status": self.split.status_json(),
        });
        json_response(200, &body.to_string())
    }

    fn api_split_set_policy(&self, body: &str) -> Response<std::io::Cursor<Vec<u8>>> {
        let new_split: SplitTunnelSettings = match serde_json::from_str(body) {
            Ok(v) => v,
            Err(e) => {
                return json_response(
                    400,
                    &serde_json::json!({"error": format!("invalid policy: {}", e)}).to_string(),
                )
            }
        };
        if let Err(e) = new_split.validate() {
            return json_response(
                400,
                &serde_json::json!({"error": format!("invalid policy: {}", e)}).to_string(),
            );
        }

        let geoip_cfg = { self.config.lock().unwrap().geoip.clone() };

        // Enabling the policy with no database present: fetch one first so
        // "select provider / enable" never results in an empty ruleset.
        if new_split.enabled && !crate::geoip::is_db_present(&geoip_cfg) {
            log::info!("[geoip] no database yet; building it before enabling split tunneling");
            match crate::geoip::rebuild_for_split_tunnel(&geoip_cfg, &new_split) {
                Ok(_) => {
                    self.split.reload_geoip();
                }
                Err(e) => {
                    return json_response(
                        500,
                        &serde_json::json!({"error": format!(
                            "could not build geoip database ({}); policy not enabled",
                            e
                        )})
                        .to_string(),
                    )
                }
            }
        }

        // Save first, then swap in-memory state (transactional semantics).
        {
            let mut cfg = self.config.lock().unwrap();
            let mut next_cfg = cfg.clone();
            next_cfg.split_tunnel = new_split.clone();
            if let Err(e) = next_cfg.save(&self.config_path) {
                return json_response(
                    500,
                    &serde_json::json!({"error": format!("failed to save config: {}", e)})
                        .to_string(),
                );
            }
            *cfg = next_cfg;
        }

        self.split.update_configs(
            new_split.clone(),
            geoip_cfg,
            self.config.lock().unwrap().tunnel.has_ipv6,
        );

        // Hot-reapply without tearing the tunnel down.
        let was_active = self.split.is_active();
        self.split.teardown();
        match self.split.apply_now() {
            Ok(()) => json_response(200, r#"{"status":"applied"}"#),
            Err(e) if !was_active && new_split.enabled => json_response(
                202,
                &serde_json::json!({"status":"saved","apply_error":e}).to_string(),
            ),
            Err(e) => json_response(500, &serde_json::json!({"error": e}).to_string()),
        }
    }

    fn api_split_test(&self, body: &str) -> Response<std::io::Cursor<Vec<u8>>> {
        let parsed: serde_json::Value = match serde_json::from_str(body) {
            Ok(v) => v,
            Err(_) => return json_response(400, r#"{"error":"invalid json"}"#),
        };
        let target = parsed.get("target").and_then(|v| v.as_str()).unwrap_or("");
        match self.split.test_target(target) {
            Ok(result) => json_response(200, &result.to_string()),
            Err(e) => json_response(400, &serde_json::json!({"error": e}).to_string()),
        }
    }
}

fn json_response(status: u16, body: &str) -> Response<std::io::Cursor<Vec<u8>>> {
    let data = body.as_bytes().to_vec();
    Response::from_data(data)
        .with_status_code(StatusCode(status))
        .with_header(Header::from_bytes("Content-Type", "application/json").unwrap())
        .with_header(Header::from_bytes("Access-Control-Allow-Origin", "*").unwrap())
}

fn read_body(request: &mut Request) -> Result<String, String> {
    let mut reader = request.as_reader().take((MAX_BODY_BYTES + 1) as u64);
    let mut body = String::new();
    reader
        .read_to_string(&mut body)
        .map_err(|e| format!("failed to read request body: {}", e))?;
    if body.len() > MAX_BODY_BYTES {
        return Err(format!(
            "request body too large (max {} bytes)",
            MAX_BODY_BYTES
        ));
    }
    Ok(body)
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
