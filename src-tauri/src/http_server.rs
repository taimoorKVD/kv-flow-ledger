use crate::db::Db;
use crate::models::HookEventIn;
use crate::services::ingest;
use serde_json::json;
use std::io::Read;
use std::sync::Arc;
use std::thread;
use tiny_http::{Header, Method, Response, Server, StatusCode};

fn header_is(name: &str, expected: &str) -> bool {
    name.eq_ignore_ascii_case(expected)
}

pub fn start_ingest_server(db: Arc<Db>, port: u16, token: String) -> std::thread::JoinHandle<()> {
    thread::spawn(move || {
        let addr = format!("127.0.0.1:{port}");
        let server = match Server::http(&addr) {
            Ok(s) => s,
            Err(e) => {
                tracing::error!("ingest server failed to bind {addr}: {e}");
                return;
            }
        };
        tracing::info!("FlowLedger ingest listening on http://{addr}/hooks/ingest");
        for mut request in server.incoming_requests() {
            let url = request.url().to_string();
            let method = request.method().clone();
            if method == Method::Get && (url == "/health" || url.starts_with("/health?")) {
                let body = json!({"ok": true, "service": "flowledger"}).to_string();
                let _ = request.respond(Response::from_string(body).with_header(
                    Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap(),
                ));
                continue;
            }
            if method == Method::Post && url.starts_with("/hooks/ingest") {
                if !token.is_empty() {
                    let expected = format!("Bearer {token}");
                    let auth_ok = request.headers().iter().any(|h| {
                        header_is(h.field.as_str().as_str(), "authorization")
                            && h.value.as_str() == expected
                    });
                    if !auth_ok {
                        let _ = request.respond(
                            Response::from_string(json!({"error":"unauthorized"}).to_string())
                                .with_status_code(StatusCode(401)),
                        );
                        continue;
                    }
                }
                let mut body = String::new();
                if request.as_reader().read_to_string(&mut body).is_err() {
                    let _ = request.respond(
                        Response::from_string(json!({"error":"bad body"}).to_string())
                            .with_status_code(StatusCode(400)),
                    );
                    continue;
                }
                let outcome = match serde_json::from_str::<HookEventIn>(&body) {
                    Ok(ev) => ingest::ingest_event(&db, ev),
                    Err(e) => {
                        let _ = request.respond(
                            Response::from_string(json!({"error": e.to_string()}).to_string())
                                .with_status_code(StatusCode(400)),
                        );
                        continue;
                    }
                };
                match outcome {
                    Ok(o) => {
                        let status = if o.duplicate { 200 } else { 201 };
                        let _ = request.respond(
                            Response::from_string(serde_json::to_string(&o).unwrap_or_default())
                                .with_status_code(StatusCode(status))
                                .with_header(
                                    Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..])
                                        .unwrap(),
                                ),
                        );
                    }
                    Err(e) => {
                        tracing::warn!("ingest error: {e}");
                        let _ = request.respond(
                            Response::from_string(json!({"error": e.to_string()}).to_string())
                                .with_status_code(StatusCode(500)),
                        );
                    }
                }
                continue;
            }
            let _ = request.respond(Response::from_string("not found").with_status_code(StatusCode(404)));
        }
    })
}
