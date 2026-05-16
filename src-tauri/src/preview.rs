//! Mini-Preview-Server: axum, statisch, ein Worker für alle Projekte.
//!
//! Der `serve_root` wird beim Öffnen/Bauen eines Projekts gesetzt. Bis dann
//! liefert der Server 503.
//!
//! Zusätzlich exponiert der Server `/__reload` als SSE-Endpoint. Beim Ausliefern
//! von HTML-Antworten wird ein winziges Reload-Snippet vor `</body>` injiziert,
//! das auf `event: reload` lauscht und `location.reload()` aufruft. So kann das
//! Frontend nach einem erfolgreichen Build den geöffneten Browser-Tab auffrischen.

use axum::{
    body::Body,
    extract::State,
    http::{header, Request, StatusCode},
    response::{
        sse::{Event, KeepAlive, Sse},
        Response,
    },
    routing::{any, get},
    Router,
};
use camino::Utf8PathBuf;
use futures_util::stream::Stream;
use std::{
    convert::Infallible,
    net::SocketAddr,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::sync::broadcast;
use tokio_stream::{wrappers::BroadcastStream, StreamExt};

const RELOAD_SNIPPET: &str = r#"<script>(function(){try{var es=new EventSource('/__reload');es.addEventListener('reload',function(){location.reload();});}catch(e){}})();</script>"#;

#[derive(Clone)]
pub struct PreviewState {
    pub serve_root: Arc<Mutex<Option<Utf8PathBuf>>>,
    pub reload_tx: broadcast::Sender<()>,
}

impl Default for PreviewState {
    fn default() -> Self {
        let (tx, _) = broadcast::channel(16);
        Self {
            serve_root: Arc::new(Mutex::new(None)),
            reload_tx: tx,
        }
    }
}

impl PreviewState {
    pub fn set_root(&self, root: Utf8PathBuf) {
        *self.serve_root.lock().unwrap() = Some(root);
    }

    pub fn notify_reload(&self) {
        // Ein Send-Fehler bedeutet schlicht: kein Abonnent. Nicht tragisch.
        let _ = self.reload_tx.send(());
    }
}

pub async fn start(state: PreviewState) -> std::io::Result<u16> {
    let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let listener = tokio::net::TcpListener::bind(addr).await?;
    let port = listener.local_addr()?.port();

    let app = Router::new()
        .route("/__reload", get(reload_sse))
        .fallback(any(serve))
        .with_state(state);

    tauri::async_runtime::spawn(async move {
        if let Err(e) = axum::serve(listener, app).await {
            eprintln!("preview server stopped: {e}");
        }
    });

    Ok(port)
}

async fn reload_sse(
    State(state): State<PreviewState>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let rx = state.reload_tx.subscribe();
    let stream = BroadcastStream::new(rx).filter_map(|res| match res {
        Ok(()) => Some(Ok(Event::default().event("reload").data("1"))),
        Err(_lag) => None,
    });
    Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(15)))
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
            let body_bytes = if mime.starts_with("text/html") {
                inject_reload(&bytes)
            } else {
                bytes
            };
            Response::builder()
                .header(header::CONTENT_TYPE, mime)
                .header(header::CACHE_CONTROL, "no-store")
                .body(Body::from(body_bytes))
                .unwrap()
        }
        Err(_) => {
            // Serve 404.html from build root if present, otherwise default text
            let four_oh_four = root.join("404.html");
            if let Ok(bytes) = tokio::fs::read(four_oh_four.as_std_path()).await {
                let body_bytes = inject_reload(&bytes);
                return Response::builder()
                    .status(StatusCode::NOT_FOUND)
                    .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
                    .body(Body::from(body_bytes))
                    .unwrap();
            }
            text(StatusCode::NOT_FOUND, "404")
        }
    }
}

fn inject_reload(bytes: &[u8]) -> Vec<u8> {
    // Wenn kein gültiges UTF-8 oder kein </body>, originale Bytes zurückgeben.
    let Ok(s) = std::str::from_utf8(bytes) else {
        return bytes.to_vec();
    };
    // Suche case-insensitive nach </body>; falls nicht vorhanden, hänge das Snippet ans Ende.
    let lower = s.to_ascii_lowercase();
    if let Some(idx) = lower.rfind("</body>") {
        let mut out = String::with_capacity(s.len() + RELOAD_SNIPPET.len());
        out.push_str(&s[..idx]);
        out.push_str(RELOAD_SNIPPET);
        out.push_str(&s[idx..]);
        out.into_bytes()
    } else {
        let mut out = s.to_string();
        out.push_str(RELOAD_SNIPPET);
        out.into_bytes()
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

#[cfg(test)]
mod tests {
    use super::{inject_reload, sanitize_path, RELOAD_SNIPPET};

    #[test]
    fn strips_leading_slash() {
        assert_eq!(sanitize_path("/about/"), "about");
        assert_eq!(sanitize_path("/index.html"), "index.html");
    }

    #[test]
    fn collapses_redundant_segments() {
        assert_eq!(sanitize_path("/a//b/./c/"), "a/b/c");
        assert_eq!(sanitize_path("///"), "");
        assert_eq!(sanitize_path("/"), "");
    }

    #[test]
    fn blocks_path_traversal() {
        // Klassisches `..` darf nicht über die Root hinausführen.
        assert_eq!(sanitize_path("/../etc/passwd"), "etc/passwd");
        assert_eq!(sanitize_path("/a/../../etc/passwd"), "etc/passwd");
        assert_eq!(sanitize_path("/a/b/../../.."), "");
    }

    #[test]
    fn keeps_dotfiles_intact() {
        // Eine einzelne führende Punkt-Komponente (z.B. `.well-known`) ist legitim.
        assert_eq!(sanitize_path("/.well-known/foo"), ".well-known/foo");
    }

    #[test]
    fn injects_snippet_before_body_close() {
        let html = b"<html><body><p>hi</p></body></html>";
        let out = inject_reload(html);
        let s = std::str::from_utf8(&out).unwrap();
        assert!(s.contains(RELOAD_SNIPPET));
        let snippet_pos = s.find(RELOAD_SNIPPET).unwrap();
        let body_close = s.find("</body>").unwrap();
        assert!(snippet_pos < body_close);
    }

    #[test]
    fn appends_snippet_when_no_body_tag() {
        let html = b"<html><p>hi</p></html>";
        let out = inject_reload(html);
        let s = std::str::from_utf8(&out).unwrap();
        assert!(s.ends_with(RELOAD_SNIPPET));
    }

    #[test]
    fn injects_only_once_at_last_body_close() {
        // Falls jemand absichtlich </body> im Text hat, fügen wir vor dem LETZTEN ein.
        let html = b"<html><body>foo</body><body>bar</body></html>";
        let out = inject_reload(html);
        let s = std::str::from_utf8(&out).unwrap();
        assert_eq!(s.matches(RELOAD_SNIPPET).count(), 1);
        let snippet_pos = s.find(RELOAD_SNIPPET).unwrap();
        let last_body_close = s.rfind("</body>").unwrap();
        assert!(snippet_pos < last_body_close);
    }

    #[test]
    fn case_insensitive_body_match() {
        let html = b"<HTML><BODY>hi</BODY></HTML>";
        let out = inject_reload(html);
        let s = std::str::from_utf8(&out).unwrap();
        assert!(s.contains(RELOAD_SNIPPET));
    }
}
