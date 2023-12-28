#![feature(type_alias_impl_trait)]
#![feature(trait_alias)]

fn main() {
    #[cfg(feature = "frontend")]
    frontend::main();
    #[cfg(feature = "backend")]
    backend::main().unwrap();
}

#[cfg(feature = "frontend")]
mod frontend {
    pub fn main() {}
}

#[cfg(feature = "backend")]
mod backend {
    use axum::{middleware, Router};
    use db::{db, Database, Session, Set, User};
    use dubs::html::RenderExt;
    use dubs::{
        and, app, asc, async_trait, desc, eq, etag_middleware, res, tokio, Cache, CacheType,
        Cookie, Css, FromRequestParts, HeaderValue, IntoResponse, Js, Json, JustError, Parts,
        Responder, Response, StaticFiles, StatusCode, TypedHeader,
    };
    use dubs::{thiserror, ulid};
    use enum_router::Routes;
    use parts::*;
    use serde::{Deserialize, Serialize};
    use std::borrow::Cow;

    #[tokio::main]
    pub async fn main() -> Result<()> {
        db().await;
        app()
            .routes(routes())
            .static_files(StaticFile::once())
            .serve("127.0.0.1:9005")
            .await;

        Ok(())
    }

    fn routes() -> Router {
        Route::router().layer(middleware::from_fn(etag_middleware))
    }

    type Html = Result<Responder>;

    async fn root(SomeUser(user): SomeUser) -> Html {
        let is_logged_in = &user.as_ref().is_some();
        let user_id = user.unwrap_or_default().id;
        let Database { db, sets, .. } = db().await;
        let sets: Vec<Set> = db
            .select()
            .from(sets)
            .r#where(eq(sets.user_id, &user_id))
            .limit(30)
            .order(vec![asc(sets.name)])
            .all()
            .await?;
        let mut names = sets.into_iter().map(|s| s.name).collect::<Vec<_>>();
        names.dedup();

        render(
            Route::Root,
            root_part(is_logged_in, names, SetForm::default()),
        )
    }

    async fn create_set(
        user: Option<User>,
        Json(form): Json<SetForm>,
    ) -> Result<impl IntoResponse> {
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
                    .r#where(eq(sets.user_id, &user.id))
                    .order(vec![desc(sets.created_at)])
                    .limit(30)
                    .all()
                    .await?;

                Ok(res()
                    .redirect(Route::SetList)
                    .set_cookie(session_cookie(Some(session.id)))
                    .render(set_list_part(user, sets)))
            }
        }
    }

    async fn set_list(user: User) -> Html {
        let Database { db, sets, .. } = db().await;

        let sets: Vec<Set> = db
            .select()
            .from(sets)
            .r#where(eq(sets.user_id, &user.id))
            .order(vec![desc(sets.created_at)])
            .limit(30)
            .all()
            .await?;

        render(Route::SetList, set_list_part(user, sets))
    }

    async fn profile(user: User) -> impl IntoResponse {
        response(Route::Profile, profile_part(user)).cache(Cache {
            max_age: 60,
            cache_type: CacheType::Private,
            must_revalidate: false,
            no_cache: false,
        })
    }

    async fn logout() -> impl IntoResponse {
        res().redirect(Route::Root).set_cookie(session_cookie(None))
    }

    #[derive(Serialize, Deserialize)]
    struct DeleteSetForm {
        id: String,
    }

    async fn delete_set(
        user: User,
        Json(DeleteSetForm { id }): Json<DeleteSetForm>,
    ) -> Result<impl IntoResponse> {
        let Database { db, sets, .. } = db().await;
        let _ = db
            .delete_from(sets)
            .r#where(and(eq(sets.id, id), eq(sets.user_id, user.id)))
            .rows_affected()
            .await?;

        Ok(res().redirect(Route::SetList))
    }

    async fn login_form() -> Html {
        render(
            Route::Login,
            login_form_part(LoginForm {
                secret: "".into(),
                error: None,
            }),
        )
    }

    #[derive(Serialize, Deserialize, Clone)]
    struct LoginForm {
        secret: String,
        error: Option<String>,
    }

    async fn login(Json(params): Json<LoginForm>) -> Result<impl IntoResponse> {
        let Database {
            db,
            users,
            sessions,
            ..
        } = db().await;
        let user: Option<User> = db
            .select()
            .from(users)
            .r#where(eq(users.secret, &params.secret))
            .first()
            .await
            .ok();
        match user {
            Some(user) => {
                let session: Session = db
                    .insert(sessions)
                    .values(Session::new(&user))?
                    .returning()
                    .await?;
                Ok(res()
                    .redirect(Route::Root)
                    .set_cookie(session_cookie(Some(session.id)))
                    .into_response())
            }
            None => Ok(render(Route::LoginForm, login_form_part(params.clone())).into_response()),
        }
    }

    mod parts {

        use super::*;
        use dubs::{
            html::{self, *},
            Responder,
        };

        pub trait Render = dubs::html::Render + 'static;

        pub fn root_part(
            is_logged_in: &bool,
            names: Vec<String>,
            set_form: SetForm,
        ) -> impl Render {
            div.class("flex flex-col gap-8")((
                h1.class("text-2xl text-center")("u lift bro?"),
                set_form_part(names, set_form),
                render_if(
                    !*is_logged_in,
                    a.class("text-center").href(Route::LoginForm)("Already have an account?"),
                ),
            ))
        }

        fn set_form_part(
            names: Vec<String>,
            SetForm { name, reps, weight }: SetForm,
        ) -> impl Render {
            form(Route::CreateSet).class("flex flex-col px-4 lg:px-0 pt-4 gap-4")((
                div((label("exercise"), suggest_input("name", names, name, true))),
                div.class("flex gap-4")((
                    div.class("w-full")((label("reps"), number_input("reps", reps))),
                    div.class("w-full")((label("weight"), number_input("weight", weight))),
                )),
                button()("save your set"),
            ))
        }

        fn render_if(is_true: bool, part: impl Render) -> impl Render {
            raw(if is_true {
                part.render_to_string()
            } else {
                String::with_capacity(0)
            })
        }

        fn time_ago(seconds: u64) -> impl Render {
            let now = now();
            let seconds = now - seconds;

            const YEAR: u64 = 31_536_000;
            const MONTH: u64 = 2_592_000;
            const DAY: u64 = 86_400;
            const HOUR: u64 = 3600;
            const MINUTE: u64 = 60;
            let diff = seconds / YEAR;
            if diff > 0 {
                return format!("{}y ago", diff);
            }

            let diff = seconds / MONTH;
            if diff > 0 {
                return format!("{}m ago", diff);
            }

            let diff = seconds / DAY;
            if diff > 0 {
                return format!("{}d ago", diff);
            }

            let diff = seconds / HOUR;
            if diff > 0 {
                return format!("{}h ago", diff);
            }

            let diff = seconds / MINUTE;
            if diff > 0 {
                return format!("{}m ago", diff);
            }

            return format!("{}s ago", seconds);
        }

        fn seconds_ago(seconds: u64) -> u64 {
            let now = now();
            now - seconds
        }

        fn set_li(set: Set) -> impl Render {
            li.class("flex justify-between")((
                div.class("flex flex-col gap-1 py-5")((
                    div.class("font-bold")(set.name),
                    div.class("flex gap-4 dark:text-gray-400 text-gray-300")((
                        render_if(set.weight != 0, span((set.weight, " lbs"))),
                        span((set.reps, " reps")),
                        time_ago(set.created_at),
                    )),
                )),
                form(Route::DeleteSet).class("flex justify-center items-center")((
                    hidden_input().name("id").value(set.id),
                    small_button()("Delete"),
                )),
            ))
        }

        fn hidden_input() -> Tag {
            input.r#type("hidden")
        }

        fn small_button() -> Tag {
            html::button
            .class("rounded-md bg-transparent border dark:border-gray-700 border-gray-300 text-white px-3 py-1")
        }

        pub fn set_list_part(user: User, sets: Vec<Set>) -> impl Render {
            div.class("px-4 lg:px-0 flex flex-col gap-4")((
                h1.class("text-2xl text-center")("Sets"),
                render_if(
                    seconds_ago(user.created_at) < 60,
                    div.class("bg-gray-300 dark:bg-gray-800 p-4 rounded-md flex flex-col gap-3")((
                        div.class("flex flex-col gap-1")((
                            p("this is your secret"),
                            p.class("font-bold")(user.secret),
                            p("it's the only way to log back in, don't lose it!"),
                            span((
                                span("you can always see your secret in "),
                                a.class("underline").href(Route::Profile)("profile"),
                            )),
                        )),
                        p("this message will self-destruct in 60s"),
                    )),
                ),
                ul.class("divide-y divide-gray-100 dark:divide-gray-800")(
                    sets.into_iter().map(set_li).collect::<Vec<_>>(),
                ),
                div.class("invisible lg:visible")(link_button().href(Route::Root)(
                    "start another set",
                )),
            ))
        }

        pub fn link_button() -> Tag {
            a.class("flex rounded-md bg-orange-500 active:bg-orange-700 text-white p-4 items-center justify-center uppercase w-full")
        }

        pub fn button() -> Tag {
            html::button.class("flex rounded-md bg-orange-500 active:bg-orange-700 text-white p-4 items-center justify-center uppercase w-full")
        }

        fn head() -> impl Render {
            let static_files = StaticFile::once();
            html::head((
                link.href(static_files.tailwind.clone()).rel("stylesheet"),
                script.src(static_files.htmx.clone()).defer(),
                script.src(static_files.json_enc.clone()).defer(),
                script.src(static_files.preload.clone()).defer(),
                meta.charset("UTF-8"),
                meta.content("text/html; charset=utf-8")
                    .attr("http-equiv", "Content-Type"),
                meta.name("viewport")
                    .content("width=device-width, initial-scale=1, user-scalable=no"),
            ))
        }

        fn plus_circle_icon() -> impl Render {
            raw(r#"
            <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor" data-slot="icon" class="w-6 h-6">
              <path stroke-linecap="round" stroke-linejoin="round" d="M12 9v6m3-3H9m12 0a9 9 0 1 1-18 0 9 9 0 0 1 18 0Z" />
            </svg>
        "#)
        }

        fn list_icon() -> impl Render {
            raw(
                r#"<svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor" data-slot="icon" class="w-6 h-6">
  <path stroke-linecap="round" stroke-linejoin="round" d="M8.25 6.75h12M8.25 12h12m-12 5.25h12M3.75 6.75h.007v.008H3.75V6.75Zm.375 0a.375.375 0 1 1-.75 0 .375.375 0 0 1 .75 0ZM3.75 12h.007v.008H3.75V12Zm.375 0a.375.375 0 1 1-.75 0 .375.375 0 0 1 .75 0Zm-.375 5.25h.007v.008H3.75v-.008Zm.375 0a.375.375 0 1 1-.75 0 .375.375 0 0 1 .75 0Z" />
</svg>
"#,
            )
        }

        fn user_circle_icon() -> impl Render {
            raw(
                r#"<svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor" data-slot="icon" class="w-6 h-6">
  <path stroke-linecap="round" stroke-linejoin="round" d="M17.982 18.725A7.488 7.488 0 0 0 12 15.75a7.488 7.488 0 0 0-5.982 2.975m11.963 0a9 9 0 1 0-11.963 0m11.963 0A8.966 8.966 0 0 1 12 21a8.966 8.966 0 0 1-5.982-2.275M15 9.75a3 3 0 1 1-6 0 3 3 0 0 1 6 0Z" />
</svg>
"#,
            )
        }

        fn nav_link(
            route: Route,
            current_route: Route,
            icon: impl Render,
            s: &'static str,
        ) -> impl Render {
            let mut class = "flex flex-col text-center justify-center items-center".to_owned();
            if route == current_route {
                class.push_str(" text-orange-500");
            }
            a.class(class).attr1("preload").href(route)((
                span.class("visible lg:invisible")(icon),
                span(s),
            ))
        }

        fn nav(route: Route) -> impl Render {
            html::nav.class(
            "text-center lg:max-w-md w-full dark:bg-gray-800 bg-gray-300 lg:bg-transparent lg:dark:bg-transparent flex lg:mx-auto justify-around items-center lg:py-6 py-2 absolute bottom-0 lg:bottom-auto lg:relative",
        )((
            nav_link(Route::SetList, route, list_icon(), "Sets"),
            nav_link(Route::Root, route, plus_circle_icon(), "Add a set"),
            nav_link(Route::Profile, route, user_circle_icon(), "Profile"),
        ))
        }

        fn body(route: Route, inner: impl Render) -> impl Render {
            html::body
                .class("h-screen dark:bg-gray-900 dark:text-white")
                .attr("hx-boost", "true")
                .attr("hx-push-url", "true")
                .attr("hx-ext", "json-enc, preload")((
                nav(route),
                html::main.class("max-w-lg mx-auto lg:mt-16 pt-4")(inner),
            ))
        }

        pub fn render(route: Route, inner: impl Render) -> Result<Responder> {
            Ok(response(route, inner))
        }

        pub fn response(route: Route, inner: impl Render) -> Responder {
            res().render((doctype("html"), html((head(), body(route, inner)))))
        }

        fn label(name: &'static str) -> impl Render {
            html::label.class("flex flex-col gap-1").r#for(name)(name)
        }

        fn text_input() -> Tag {
            input.class("block w-full rounded-md border-0 px-2 py-4 dark:bg-gray-700 dark:text-white light:text-gray-900 outline-0 focus:outline-0 focus:ring-0 focus-visible:outline-0 focus:outline-none placeholder:text-gray-400").r#type("text")
        }

        fn number_input(name: &'static str, value: usize) -> impl Render {
            input.class("block w-full rounded-md border-0 px-2 py-4 dark:bg-gray-700 dark:text-white light:text-gray-900 outline-0 focus:outline-0 focus:ring-0 focus-visible:outline-0 focus:outline-none placeholder:text-gray-400").r#type("number").name(name).value(value.to_string())
        }

        fn suggest_input(
            name: &'static str,
            options: Vec<String>,
            value: String,
            autofocus: bool,
        ) -> impl Render {
            let list = format!("{}_list", name);
            let mut text_input = text_input()
                .name(name)
                .value(value)
                .id(name)
                .attr("list", list.clone());
            if autofocus {
                text_input = text_input.attr("autofocus", "autofocus")
            }
            (
                text_input,
                datalist.id(list)(
                    options
                        .into_iter()
                        .map(|s| option.value(s))
                        .collect::<Vec<_>>(),
                ),
            )
        }

        pub fn profile_part(user: User) -> impl Render {
            div.class("flex flex-col gap-8 px-4 lg:px-0")((
                h1.class("text-2xl text-center")("Profile"),
                h1(("Your secret key: ", span.class("font-bold")(user.secret))),
                p("don't lose this, it's your only way back to your sets"),
                form(Route::Logout)(button()("logout")),
            ))
        }

        fn form(route: Route) -> Tag {
            html::form.method("post").action(route)
        }

        pub fn login_form_part(login_form: LoginForm) -> impl Render {
            form(Route::Login).class("flex flex-col gap-4 px-4 lg:px-0")((
                div(match login_form.error {
                    Some(err) => err,
                    None => "".into(),
                }),
                div.class("flex flex-col gap-1")((
                    label("Enter your secret"),
                    text_input()
                        .attr("autofocus", "autofocus")
                        .name("secret")
                        .value(login_form.secret),
                )),
                button()("login"),
            ))
        }
    }

    #[derive(Serialize, Deserialize, Default)]
    struct SetForm {
        name: String,
        reps: usize,
        weight: usize,
    }

    #[derive(StaticFiles)]
    struct StaticFile {
        #[file("/static/htmx.js")]
        htmx: Js,
        #[file("/static/tailwind.css")]
        tailwind: Css,
        #[file("/static/json-enc.js")]
        json_enc: Js,
        #[file("/static/preload.js")]
        preload: Js,
    }

    #[derive(Routes, PartialEq, Debug, Clone, Copy)]
    enum Route {
        #[get("/")]
        Root,
        #[post("/sets")]
        CreateSet,
        #[get("/sets")]
        SetList,
        #[get("/profile")]
        Profile,
        #[post("/delete-set")]
        DeleteSet,
        #[post("/logout")]
        Logout,
        #[get("/login")]
        LoginForm,
        #[post("/login")]
        Login,
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
        use crate::backend::*;
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

        #[derive(Serialize, Deserialize, Debug, Default)]
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
        pub struct Session {
            pub id: String,
            pub user_id: String,
            pub created_at: u64,
        }

        #[derive(Clone, Serialize, Deserialize, Debug)]
        pub struct Set {
            pub id: String,
            pub user_id: String,
            pub name: String,
            pub weight: usize,
            pub reps: usize,
            pub created_at: u64,
        }

        impl Set {
            pub fn new(user: &User, name: String, reps: usize, weight: usize) -> Self {
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
            &format!("id={}", id.as_ref().unwrap_or(&"".to_owned())),
            "HttpOnly",
            &format!(
                "Max-Age={}",
                match id.as_ref() {
                    Some(_) => 34_560_000,
                    None => 0,
                }
            ),
            "SameSite=Strict",
            #[cfg(not(debug_assertions))]
            "Secure",
            "Path=/",
        ]
        .join(";");
        HeaderValue::from_str(&format!("{}", parts)).unwrap()
    }
}
