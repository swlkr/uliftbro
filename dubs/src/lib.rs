use std::fmt::Display;

pub use axum::http::Uri;
pub use axum::middleware;
pub use axum::{
    async_trait,
    body::Body,
    extract::{FromRequestParts, Json, Query},
    http::header::*,
    http::request::Parts,
    http::StatusCode,
    response::{AppendHeaders, IntoResponse, Response},
    routing::{self, get, post},
    RequestPartsExt, Router,
};
use axum::{extract::Request, middleware::Next};
pub use axum_extra::headers::Cookie;
pub use axum_extra::typed_header::TypedHeaderRejection;
pub use axum_extra::TypedHeader;
pub use justerror::Error as JustError;
pub use static_stash::{Css, Js, StaticFiles, Wasm};
use stpl::html::RenderExt;
use stpl::Render;
pub use thiserror;
pub mod tokio {
    pub use tokio::*;
}
pub use axum::response::Redirect;
pub use rizz::{
    self, and, asc, connection, desc, eq, like, or, r#in, Blob, Connection, Database, Integer,
    JournalMode, Migrator, Real, Synchronous, Table, Text,
};
pub use serde::*;

pub fn ulid() -> String {
    ulid::Ulid::new().to_string()
}

pub mod html {
    use std::fmt::Display;

    use axum::body::Body;
    use axum::http::header::CONTENT_TYPE;
    use axum::http::StatusCode;
    use axum::response::{IntoResponse, Response};
    pub use stpl::html::RenderExt;
    pub use stpl::html::{
        a, b, blockquote, body, button, datalist, div, doctype, footer, form, h1, h2, h3, h4, h5,
        head, html, i, img, input, label, li, link, main, meta, nav, ol, option, p, pre, raw,
        script, section, span, string, tbody, textarea, th, thead, tr, tt, u, ul, BareTag,
        FinalTag, Tag,
    };
    pub use stpl::Render;
    pub use stpl::Renderer;

    pub struct Html(pub Box<dyn Render>);

    impl Display for Html {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_fmt(format_args!("{}", self.0.render_to_string()))
        }
    }

    pub fn render(children: impl Render + 'static) -> Html {
        Html(Box::new(children))
    }

    impl IntoResponse for Html {
        fn into_response(self) -> axum::response::Response {
            Response::builder()
                .status(StatusCode::OK)
                .header(CONTENT_TYPE, "text/html")
                .body(Body::from(self.to_string()))
                .unwrap()
        }
    }
}

pub struct App {
    router: Router,
}

pub fn app() -> App {
    App {
        router: Router::new(),
    }
}

impl App {
    pub fn routes(mut self, router: Router) -> Self {
        self.router = router;
        self
    }

    pub fn static_files(mut self, static_files: &'static (impl StaticFiles + Send + Sync)) -> Self {
        self.router = self.router.route(
            "/*file",
            axum::routing::get(move |uri: Uri| async move {
                match static_files.get(&uri.path()) {
                    Some(file) => (
                        StatusCode::OK,
                        [
                            (CONTENT_TYPE, file.content_type),
                            (CACHE_CONTROL, "public, max-age=604800, immutable"),
                        ],
                        file.content,
                    ),
                    None => (
                        StatusCode::NOT_FOUND,
                        [
                            (CONTENT_TYPE, "text/html; charset=utf-8"),
                            (CACHE_CONTROL, "public, max-age=604800, immutable"),
                        ],
                        "not found".as_bytes().to_vec(),
                    ),
                }
            }),
        );
        self
    }

    pub async fn serve(self, ip: &str) {
        let listener = tokio::net::TcpListener::bind(ip).await.unwrap();
        println!("Listening on {}", ip);
        axum::serve(listener, self.router).await.unwrap();
    }
}

pub async fn etag_middleware(request: Request, next: Next) -> Response {
    let if_none_match_header = request.headers().get(IF_NONE_MATCH).cloned();
    let response = next.run(request).await;
    let (mut parts, body) = response.into_parts();
    let bytes = match axum::body::to_bytes(body, usize::MAX).await {
        Ok(bytes) => bytes,
        Err(_err) => return (StatusCode::BAD_REQUEST, "Failed to read body").into_response(),
    };

    let (etag, body) = match bytes.len() == 0 {
        true => return parts.into_response(),
        false => (hash(&bytes), Body::from(bytes)),
    };

    match if_none_match_header {
        Some(if_none_match) => {
            if if_none_match.to_str().unwrap() == etag {
                parts.headers.insert(ETAG, etag.parse().unwrap());
                ((StatusCode::NOT_MODIFIED, parts)).into_response()
            } else {
                (parts, body).into_response()
            }
        }
        None => (parts, body).into_response(),
    }
}

pub fn res() -> Responder {
    Responder::new()
}

impl IntoResponse for Responder {
    fn into_response(self) -> Response {
        (self.status_code, self.headers, self.body).into_response()
    }
}

pub struct Responder {
    status_code: StatusCode,
    headers: HeaderMap,
    body: Body,
}

const HX_LOCATION: HeaderName = HeaderName::from_static("hx-location");

impl Responder {
    fn new() -> Self {
        Self {
            status_code: StatusCode::OK,
            headers: HeaderMap::default(),
            body: Body::empty(),
        }
    }

    pub fn render(mut self, component: impl Render + 'static) -> Self {
        let body = component.render_to_string();

        self.headers
            .insert(ETAG, hash(body.as_bytes()).parse().unwrap());
        self.headers
            .insert(CONTENT_TYPE, "text/html; charset=utf-8".parse().unwrap());
        self.body = Body::from(body);

        self
    }

    pub fn cache(mut self, cache: Cache) -> Self {
        self.headers
            .insert(CACHE_CONTROL, cache.to_string().parse().unwrap());
        self
    }

    pub fn redirect(mut self, route: impl Display) -> Self {
        let value = HeaderValue::from_str(&route.to_string()).unwrap();
        self.headers.insert(LOCATION, value.clone());
        self.headers.insert(HX_LOCATION, value.clone());
        self
    }

    pub fn header(mut self, name: impl Into<HeaderName>, value: impl Into<HeaderValue>) -> Self {
        self.headers.insert(name.into(), value.into());
        self
    }

    pub fn set_cookie(mut self, value: impl Into<HeaderValue>) -> Self {
        self.headers.insert(SET_COOKIE, value.into());
        self
    }
}

fn hash(content: &[u8]) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    content.hash(&mut hasher);
    let hash_value = hasher.finish();

    hash_value.to_string()
}

pub enum CacheType {
    Public,
    Private,
}

impl Display for CacheType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            CacheType::Public => "public",
            CacheType::Private => "private",
        })
    }
}

pub struct Cache {
    pub max_age: u64,
    pub no_cache: bool,
    pub cache_type: CacheType,
    pub must_revalidate: bool,
}

impl Display for Cache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let parts = vec![
            Some(format!("max_age={}", self.max_age)),
            if self.no_cache {
                Some("no-cache".to_owned())
            } else {
                None
            },
            Some(self.cache_type.to_string()),
            if self.must_revalidate {
                Some("must-revalidate".to_owned())
            } else {
                None
            },
        ]
        .into_iter()
        .filter_map(|x| x)
        .collect::<Vec<String>>()
        .join(",");
        f.write_fmt(format_args!("{}", parts))
    }
}
