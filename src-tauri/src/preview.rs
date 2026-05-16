//! Mini-Preview-Server: axum, statisch, ein Worker für alle Projekte.
//!
//! Der `serve_root` wird beim Öffnen/Bauen eines Projekts gesetzt. Bis dann
//! liefert der Server 503.

use axum::{
    body::Body,
    extract::State,
    http::{header, Request, StatusCode},
    response::Response,
    routing::any,
    Router,
};
use camino::Utf8PathBuf;
use std::{
    net::SocketAddr,
    sync::{Arc, Mutex},
};

#[derive(Clone, Default)]
pub struct PreviewState {
    pub serve_root: Arc<Mutex<Option<Utf8PathBuf>>>,
}

impl PreviewState {
    pub fn set_root(&self, root: Utf8PathBuf) {
        *self.serve_root.lock().unwrap() = Some(root);
    }
}

pub async fn start(state: PreviewState) -> std::io::Result<u16> {
    let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let listener = tokio::net::TcpListener::bind(addr).await?;
    let port = listener.local_addr()?.port();

    let app = Router::new().fallback(any(serve)).with_state(state);

    tauri::async_runtime::spawn(async move {
        if let Err(e) = axum::serve(listener, app).await {
            eprintln!("preview server stopped: {e}");
        }
    });

    Ok(port)
}

async fn serve(State(state): State<PreviewState>, req: Request<Body>) -> Response {
    let Some(root) = state.serve_root.lock().unwrap().clone() else {
        return text(StatusCode::SERVICE_UNAVAILABLE, "kein Projekt geöffnet — bitte erst bauen");
    };

    let path = sanitize_path(req.uri().path());
    let mut full = root.join(&path);

    if full.is_dir() {
        full = full.join("index.html");
    } else if !full.exists() {
        // Try appending index.html if URL has trailing-slash semantics
        let candidate = root.join(path.trim_end_matches('/')).join("index.html");
        if candidate.exists() {
            full = candidate;
        }
    }

    match tokio::fs::read(full.as_std_path()).await {
        Ok(bytes) => {
            let mime = mime_guess::from_path(full.as_std_path())
                .first_or_octet_stream()
                .to_string();
            Response::builder()
                .header(header::CONTENT_TYPE, mime)
                .header(header::CACHE_CONTROL, "no-store")
                .body(Body::from(bytes))
                .unwrap()
        }
        Err(_) => {
            // Serve 404.html from build root if present, otherwise default text
            let four_oh_four = root.join("404.html");
            if let Ok(bytes) = tokio::fs::read(four_oh_four.as_std_path()).await {
                return Response::builder()
                    .status(StatusCode::NOT_FOUND)
                    .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
                    .body(Body::from(bytes))
                    .unwrap();
            }
            text(StatusCode::NOT_FOUND, "404")
        }
    }
}

fn sanitize_path(uri_path: &str) -> String {
    let trimmed = uri_path.trim_start_matches('/');
    let mut clean = Vec::new();
    for part in trimmed.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                clean.pop();
            }
            other => clean.push(other),
        }
    }
    clean.join("/")
}

fn text(status: StatusCode, msg: &str) -> Response {
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
        .body(Body::from(msg.to_string()))
        .unwrap()
}
