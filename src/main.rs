#![feature(type_alias_impl_trait)]
#![feature(trait_alias)]

use std::borrow::Cow;

use db::{db, Database, Session, Set, User};
use dubs::{
    app, asc, async_trait, desc, eq, serve, tokio, Cookie, Css, Deserialize, FromRequestParts,
    HeaderMap, HeaderName, HeaderValue, IntoResponse, Js, Json, JustError, Parts, Response, Routes,
    Serialize, StaticFiles, StatusCode, TypedHeader, CONTENT_TYPE, LOCATION, SET_COOKIE,
};
use dubs::{thiserror, ulid};
use view::*;

#[tokio::main]
async fn main() -> Result<()> {
    let app = app().routes(Route::new()).static_files(StaticFile::once());
    let _ = db().await;
    serve(app, "127.0.0.1:9001").await;

    Ok(())
}

async fn root(SomeUser(user): SomeUser) -> Result<Html> {
    let Database { db, sets, .. } = db().await;
    let sets: Vec<Set> = match user {
        Some(user) => {
            db.select()
                .from(sets)
                .r#where(eq(sets.user_id, user.id))
                .limit(30)
                .order(vec![asc(sets.name)])
                .all()
                .await?
        }
        None => vec![],
    };
    render(root_component(sets, SetForm::default()))
}

async fn create_set(user: Option<User>, Json(form): Json<SetForm>) -> Result<impl IntoResponse> {
    let Database {
        db,
        sessions,
        users,
        sets,
    } = db().await;

    match user {
        Some(user) => {
            // already logged in
            // create set
            let _: Set = db
                .insert(sets)
                .values(Set::new(&user, form.name.clone(), form.reps, form.weight))?
                .returning()
                .await?;

            Ok(res().redirect(Route::SetList))
        }
        None => {
            // create user
            let user: User = db.insert(users).values(User::new())?.returning().await?;

            // create session
            let session: Session = db
                .insert(sessions)
                .values(Session::new(&user))?
                .returning()
                .await?;

            // create set
            let _: Set = db
                .insert(sets)
                .values(Set::new(&user, form.name, form.reps, form.weight))?
                .returning()
                .await?;

            let sets: Vec<Set> = db
                .select()
                .from(sets)
                .r#where(eq(sets.user_id, user.id))
                .order(vec![desc(sets.created_at)])
                .limit(30)
                .all()
                .await?;

            Ok(res()
                .redirect(Route::SetList)
                .header(SET_COOKIE, session_cookie(Some(session.id)))
                .render(view::set_list(sets)))
        }
    }
}

async fn set_list(user: User) -> Result<Html> {
    let Database { db, sets, .. } = db().await;

    let sets: Vec<Set> = db
        .select()
        .from(sets)
        .r#where(eq(sets.user_id, user.id))
        .order(vec![desc(sets.created_at)])
        .limit(30)
        .all()
        .await?;

    render(view::set_list(sets))
}

mod view {
    use super::*;
    pub use dubs::html::Html;
    use dubs::html::{self, *};
    use std::fmt::Display;

    pub trait Render = dubs::html::Render + 'static;

    pub fn root_component(sets: Vec<Set>, SetForm { name, reps, weight }: SetForm) -> impl Render {
        (
            h1.class("text-2xl text-center")("u lift bro?"),
            ( form.action(Route::CreateSet).method("post").class("flex flex-col px-4 lg:px-0 pt-4 gap-4")((
            div((
                label("name"),
                    suggest_input("name", sets.into_iter().map(|s| s.name).collect::<Vec<_>>(), name),
            )),
                div((
                    label("reps"),
                    number_input("reps", reps)
                )),
                div((
                    label("weight"),
                    number_input("weight", weight)
                )),
            button.class("flex rounded-md bg-orange-500 active:bg-orange-700 text-white p-4 items-center justify-center uppercase w-full")("start lifting now")
        ))))
    }

    fn set_list_item(set: Set) -> impl Render {
        li.class("justify-between flex")((
            span(set.name.clone()),
            span.class("flex gap-4")((span(set.weight.to_string()), span(set.reps.to_string()))),
        ))
    }

    pub fn set_list(sets: Vec<Set>) -> impl Render {
        (
            h1.class("text-2xl text-center")("your sets"),
            ul(sets.into_iter().map(set_list_item).collect::<Vec<_>>()),
            a.href(Route::Root).class("flex rounded-md bg-orange-500 active:bg-orange-700 text-white p-4 items-center justify-center uppercase w-full")("start another set")
        )
    }

    pub fn render(inner: impl Render) -> Result<Html> {
        let static_files = StaticFile::once();
        Ok(html::render((
            doctype("html"),
            html((
                head((
                    link.href(static_files.tailwind).rel("stylesheet"),
                    script.src(static_files.htmx).defer(),
                    script.src(static_files.json_enc).defer(),
                    meta.charset("UTF-8"),
                    meta.content("text/html; charset=utf-8")
                        .attr("http-equiv", "Content-Type"),
                    meta.name("viewport")
                        .content("width=device-width, initial-scale=1, user-scalable=no"),
                )),
                body.attr("hx-boost", "true")
                    .attr("hx-ext", "json-enc")
                    .class("h-screen dark:bg-gray-900 dark:text-white")(html::main
                    .class("max-w-lg mx-auto lg:mt-16")(
                    inner
                )),
            )),
        )))
    }

    fn label(name: &'static str) -> impl Render {
        html::label.class("flex flex-col gap-1").r#for(name)(name)
    }

    fn text_input() -> Tag {
        input.class("block w-full rounded-md border-0 px-2 py-4 dark:bg-gray-700 dark:text-white light:text-gray-900 outline-0 focus:outline-0 focus:ring-0 focus-visible:outline-0 focus:outline-none placeholder:text-gray-400").r#type("text")
    }

    fn number_input(name: &str, value: impl Display) -> impl Render {
        input.class("block w-full rounded-md border-0 px-2 py-4 dark:bg-gray-700 dark:text-white light:text-gray-900 outline-0 focus:outline-0 focus:ring-0 focus-visible:outline-0 focus:outline-none placeholder:text-gray-400").r#type("number").name(name.to_string()).value(value.to_string())
    }

    fn suggest_input(name: &'static str, options: Vec<String>, value: String) -> impl Render {
        let list = format!("{}_list", name);
        (
            text_input()
                .name(name)
                .value(value)
                .id(name)
                .attr("list", list.clone()),
            datalist.id(list)(
                options
                    .into_iter()
                    .map(|s| option.value(s))
                    .collect::<Vec<_>>(),
            ),
        )
    }
}

#[derive(Serialize, Deserialize, Default)]
#[serde(crate = "dubs")]
struct SetForm {
    name: String,
    reps: u16,
    weight: u16,
}

#[derive(StaticFiles)]
struct StaticFile {
    #[file("/static/htmx.js")]
    htmx: Js,
    #[file("/static/tailwind.css")]
    tailwind: Css,
    #[file("/static/json-enc.js")]
    json_enc: Js,
}

#[derive(Routes)]
enum Route {
    #[get("/")]
    Root,
    #[post("/sets")]
    CreateSet,
    #[get("/sets")]
    SetList,
}

impl From<Route> for Cow<'static, str> {
    fn from(value: Route) -> Self {
        Cow::Owned(value.to_string())
    }
}

#[allow(unused)]
#[JustError]
pub enum Error {
    NotFound,
    Database(String),
    InternalServer,
    RowNotFound,
    UserNotFound,
}

type Result<T> = std::result::Result<T, Error>;

mod db {
    use crate::*;
    use dubs::{rizz, Connection, JournalMode};
    use dubs::{Integer, Table, Text};

    #[derive(Clone, Debug)]
    pub struct Database {
        pub db: rizz::Database,
        pub users: Users,
        pub sessions: Sessions,
        pub sets: Sets,
    }

    impl Database {
        fn new(db: rizz::Database) -> Self {
            let users = Users::new();
            let sessions = Sessions::new();
            let sets = Sets::new();

            Self {
                db,
                sets,
                sessions,
                users,
            }
        }

        async fn migrate(&self) -> Result<()> {
            let Self {
                ref db,
                users,
                sessions,
                sets,
            } = *self;

            let _ = db
                .create_table(users)
                .create_unique_index(users, vec![users.secret])
                .create_unique_index(users, vec![users.created_at])
                .create_table(sessions)
                .create_table(sets)
                .migrate()
                .await?;

            Ok(())
        }
    }

    #[allow(unused)]
    #[derive(Table, Clone, Copy, Debug)]
    #[rizz(table = "users")]
    pub struct Users {
        #[rizz(primary_key)]
        pub id: Text,
        #[rizz(not_null)]
        pub secret: Text,
        #[rizz(not_null)]
        pub created_at: Integer,
    }

    #[allow(unused)]
    #[derive(Table, Clone, Copy, Debug)]
    #[rizz(table = "sessions")]
    pub struct Sessions {
        #[rizz(primary_key)]
        pub id: Text,
        #[rizz(not_null, references = "users(id)")]
        pub user_id: Text,
        #[rizz(not_null)]
        pub created_at: Integer,
    }

    #[allow(unused)]
    #[derive(Table, Clone, Copy, Debug)]
    #[rizz(table = "sets")]
    pub struct Sets {
        #[rizz(primary_key)]
        pub id: Text,
        #[rizz(not_null, references = "users(id)")]
        pub user_id: Text,
        #[rizz(not_null)]
        pub name: Text,
        #[rizz(not_null)]
        pub weight: Integer,
        #[rizz(not_null)]
        pub reps: Integer,
        #[rizz(not_null)]
        pub created_at: Integer,
    }

    pub async fn db<'a>() -> &'a Database {
        match DB.get() {
            Some(db) => db,
            None => {
                let db = Connection::new("db.sqlite3")
                    .create_if_missing(true)
                    .journal_mode(JournalMode::Wal)
                    .foreign_keys(true)
                    .open()
                    .await
                    .expect("Could not connect to database")
                    .database();
                let db = Database::new(db);
                let _ = db.migrate().await.expect("Migrations failed");

                DB.get_or_init(|| db)
            }
        }
    }

    impl From<rizz::Error> for Error {
        fn from(value: rizz::Error) -> Self {
            match value {
                rizz::Error::RowNotFound => Error::NotFound,
                rizz::Error::Database(err) => Error::Database(err),
                _ => Error::InternalServer,
            }
        }
    }

    #[derive(Serialize, Deserialize, Debug)]
    #[serde(crate = "dubs")]
    pub struct User {
        pub id: String,
        pub secret: String,
        pub created_at: u64,
    }
    impl User {
        pub fn new() -> Self {
            Self {
                id: ulid(),
                secret: ulid(),
                created_at: now(),
            }
        }
    }

    #[derive(Serialize, Deserialize, Debug)]
    #[serde(crate = "dubs")]
    pub struct Session {
        pub id: String,
        pub user_id: String,
        pub created_at: u64,
    }

    #[derive(Serialize, Deserialize, Debug)]
    #[serde(crate = "dubs")]
    pub struct Set {
        pub id: String,
        pub user_id: String,
        pub name: String,
        pub weight: u16,
        pub reps: u16,
        pub created_at: u64,
    }
    impl Set {
        pub fn new(user: &User, name: String, reps: u16, weight: u16) -> Self {
            Self {
                id: ulid(),
                user_id: user.id.clone(),
                name,
                weight,
                reps,
                created_at: now(),
            }
        }
    }

    impl Session {
        pub fn new(user: &User) -> Self {
            Self {
                id: ulid(),
                user_id: user.id.clone(),
                created_at: now(),
            }
        }
    }
}

static DB: std::sync::OnceLock<db::Database> = std::sync::OnceLock::new();

#[async_trait]
impl<S> FromRequestParts<S> for User
where
    S: Send + Sync,
{
    type Rejection = Error;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &S,
    ) -> std::result::Result<Self, Self::Rejection> {
        let TypedHeader(cookie) = TypedHeader::<Cookie>::from_request_parts(parts, state)
            .await
            .map_err(|_| Error::NotFound)?;
        let session_id = cookie.get("id").ok_or(Error::NotFound)?;
        let Database {
            db,
            users,
            sessions,
            ..
        } = db().await;
        let session: Session = db
            .select()
            .from(sessions)
            .r#where(eq(sessions.id, session_id))
            .first()
            .await?;
        let user: User = db
            .select()
            .from(users)
            .r#where(eq(users.id, session.user_id))
            .first()
            .await?;

        Ok(user)
    }
}

pub struct SomeUser(Option<User>);

#[async_trait]
impl<S> FromRequestParts<S> for SomeUser
where
    S: Send + Sync,
{
    type Rejection = Error;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &S,
    ) -> std::result::Result<Self, Self::Rejection> {
        let user = User::from_request_parts(parts, state).await;
        Ok(match user {
            Ok(user) => SomeUser(Some(user)),
            Err(_) => SomeUser(None),
        })
    }
}

// async fn set(user: User, Json(params): Json<SetForm>) -> Html {
//     let db = db();
//     let { sets } = db;
//     let set: Set = params.into();
//     set.user = user;
//     let _ = db.insert_into(sets).values(set).rows_affected().await?;
//     let sets: Vec<Set> = db.select().from(sets).r#where(eq(sets.user_id, user.id)).limit(30).order(desc(sets.created_at)).all().await?;
//     render_sets(sets)
// }

fn not_found(error: Error) -> Response {
    (StatusCode::NOT_FOUND, error.to_string()).into_response()
}

fn internal_server_error(error: Error) -> Response {
    #[cfg(debug_assertions)]
    return (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response();
    #[cfg(not(debug_assertions))]
    return (
        StatusCode::INTERNAL_SERVER_ERROR,
        "internal server error".to_owned(),
    )
        .into_response();
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        match self {
            Error::NotFound | Error::RowNotFound => not_found(self),
            // Error::UserNotFound => render_login()Responder::default()
            //     .render(Login(0, Login::default(), "login failed. secret incorrect"))
            //     .into_response(),
            _ => internal_server_error(self),
        }
    }
}

fn now() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now();
    now.duration_since(UNIX_EPOCH).unwrap().as_secs()
}

fn session_cookie(id: Option<String>) -> HeaderValue {
    let parts = vec![
        format!("id={}", id.unwrap_or_default()).as_str(),
        "HttpOnly",
        "Max-Age=34560000",
        "SameSite=Strict",
        #[cfg(not(debug_assertions))]
        "Secure",
        "Path=/",
    ]
    .join(";");
    HeaderValue::from_str(&format!("{}", parts)).unwrap()
}

fn res() -> Responder {
    Responder::new()
}

impl IntoResponse for Responder {
    fn into_response(self) -> Response {
        (self.status_code, self.headers, self.body).into_response()
    }
}

struct Responder {
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

    fn render(mut self, component: impl view::Render) -> Self {
        self.body = Html(Box::new(component));
        self.headers
            .insert(CONTENT_TYPE, "text/html; charset=utf-8".parse().unwrap());

        self
    }

    fn redirect(mut self, route: Route) -> Self {
        let value = HeaderValue::from_str(&route.to_string()).unwrap();
        self.headers.insert(LOCATION, value.clone());
        self.headers.insert(HX_LOCATION, value.clone());
        self
    }

    fn header(mut self, name: impl Into<HeaderName>, value: impl Into<HeaderValue>) -> Self {
        self.headers.insert(name.into(), value.into());
        self
    }
}
