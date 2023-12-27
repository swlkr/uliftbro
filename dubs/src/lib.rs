use std::fmt::Display;

pub use axum::http::Uri;
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
pub use axum_extra::headers::Cookie;
pub use axum_extra::typed_header::TypedHeaderRejection;
pub use axum_extra::TypedHeader;
use html::Html;
pub use justerror::Error as JustError;
pub use static_stash::{Css, Js, StaticFiles, Wasm};
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
                        // CACHE_CONTROL, "public, max-age=604800"
                        StatusCode::OK,
                        [
                            (CONTENT_TYPE, file.content_type),
                            (CACHE_CONTROL, "public, max-age=604800"),
                        ],
                        file.content,
                    ),
                    None => (
                        StatusCode::NOT_FOUND,
                        [
                            (CONTENT_TYPE, "text/html; charset=utf-8"),
                            (CACHE_CONTROL, "public, max-age=604800"),
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
    body: Html,
}

const HX_LOCATION: HeaderName = HeaderName::from_static("hx-location");

impl Responder {
    fn new() -> Self {
        Self {
            status_code: StatusCode::OK,
            headers: HeaderMap::default(),
            body: Html(Box::new(())),
        }
    }

    pub fn render(mut self, component: impl Render + 'static) -> Self {
        self.body = Html(Box::new(component));
        self.headers
            .insert(CONTENT_TYPE, "text/html; charset=utf-8".parse().unwrap());

        self
    }

    pub fn cache(mut self) -> Self {
        self.headers
            .insert(CACHE_CONTROL, "private, max-age=15".parse().unwrap());
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
