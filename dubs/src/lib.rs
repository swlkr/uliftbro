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
pub use enum_router::Routes;
pub use justerror::Error as JustError;
pub use static_stash::{Css, Js, StaticFiles};
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
    use stpl::html::RenderExt;
    pub use stpl::html::{
        a, b, blockquote, body, button, datalist, div, doctype, footer, form, h1, h2, h3, h4, h5,
        head, html, i, img, input, label, li, link, main, meta, nav, ol, option, p, pre, raw,
        script, section, span, tbody, textarea, th, thead, tr, tt, u, ul, BareTag, FinalTag, Tag,
    };
    pub use stpl::Render;

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

pub async fn serve(app: App, ip: &str) {
    let listener = tokio::net::TcpListener::bind(ip).await.unwrap();
    println!("Listening on {}", ip);
    axum::serve(listener, app.router).await.unwrap();
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
                        "not found",
                    ),
                }
            }),
        );
        self
    }
}

// async fn file(uri: Uri) -> impl IntoResponse {
//     StaticFile(uri.path().to_string())
// }

// #[derive(rust_embed::RustEmbed)]
// #[folder = "static"]
// #[prefix = "/static/"]
// pub struct StaticFiles;

// struct StaticFile<T>(T, Box<dyn StaticFiles + 'static>);

// impl<T> IntoResponse for StaticFile<T>
// where
//     T: Into<String>,
// {
//     fn into_response(self) -> Response {
//         let path = self.0.into();

//         match StaticFile::get(path.as_str()) {
//             Some(content) => {
//                 let mime = mime_guess::from_path(path).first_or_octet_stream();
//                 ([(header::CONTENT_TYPE, mime.as_ref())], content.data).into_response()
//             }
//             None => (StatusCode::NOT_FOUND, "404 Not Found").into_response(),
//         }
//     }
// }

// #[cfg(test)]
// mod tests {
//     use super::*;

//     #[test]
//     fn it_works() {}
// }
