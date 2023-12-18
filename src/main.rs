use dubs::{app, serve, Css, Js, Routes, StaticFiles};
use view::*;

fn main() {
    let app = app().routes(Route::new()).static_files(StaticFile::once());
    serve(app, "127.0.0.1:9001");
}

async fn root() -> Html {
    render_root(Route::Root)
}

#[derive(Routes)]
enum Route {
    #[get("/")]
    Root,
}

#[derive(StaticFiles)]
struct StaticFile {
    #[file("/static/htmx.js")]
    htmx: Js,
    #[file("/static/tailwind.css")]
    tailwind: Css,
}

mod view {
    use super::*;
    pub use dubs::html::{self, *};

    pub fn render_root(s: Route) -> Html {
        render(h1.class("text-2xl")(s.to_string()))
    }

    fn render(inner: impl Render + 'static) -> Html {
        let static_files = StaticFile::once();
        html::render((
            doctype("html"),
            html((
                head((
                    link.href(static_files.tailwind).rel("stylesheet"),
                    script.src(static_files.htmx).defer(),
                )),
                body(inner),
            )),
        ))
    }
}

// #[derive(Serialize, Deserialize)]
// #[serde(crate = "dubs")]
// // #[params]
// struct SetForm {
//     name: String,
//     reps: u16,
//     weight: u16,
// }

// async fn set_form(_: User, Query(params): Query<SetForm>) -> Html {
//     render_set_form(params)
// }

// async fn set(user: User, Json(params): Json<SetForm>) -> Html {
//     let db = db();
//     let { sets } = db;
//     let set: Set = params.into();
//     set.user = user;
//     let _ = db.insert_into(sets).values(set).rows_affected().await?;
//     let sets: Vec<Set> = db.select().from(sets).r#where(eq(sets.user_id, user.id)).limit(30).order(desc(sets.created_at)).all().await?;
//     render_sets(sets)
// }

// mod view {
//     use super::*;

//     pub fn render_root(s: Route) -> Html {
//         render(h1.class("text-2xl")(s.to_string()))
//     }

//     pub fn render_set_form(params: SetForm) -> Html {
//         hype(form((
//             input.r#type("text").name("name").value(params.name),
//             input
//                 .r#type("number")
//                 .name("reps")
//                 .value(params.reps.to_string()),
//             input
//                 .r#type("number")
//                 .name("weight")
//                 .value(params.reps.to_string()),
//         )))
//     }

//     pub fn render_sets(sets: Vec<Set>) -> Html {
//         hype(ul(sets.iter().map(render_set).collect::<Vec<Html>>()))
//     }

//     pub fn render_set(set: Set) -> Html {
//         hype
//     }

//     fn render(inner: impl Render + 'static) -> Html {
//         hype((
//             doctype("html"),
//             html((
//                 head(link.rel("stylesheet").href("/static/tailwind.css")),
//                 body(inner),
//             )),
//         ))
//     }
// }

// // #[db]
// struct Database {
//     // users: Users,
// }

// // #[table]
// // struct Users {
// //     #[db(pk, not_null)]
// //     id: Integer,
// //     #[db(unique, not_null)]
// //     secret: Text,
// //     #[db(not_null)]
// //     created_at: Integer
// // }

// // #[row]
// // struct User {
// //     id: u64,
// //     secret: String,
// //     created_at: u64
// // }

// static DB: OnceLock<Database> = OnceLock::new();
// fn db<'a>() -> &'a Database {
//     DB.get().unwrap()
// }

// // embed static files in binary
// // get hash of file contents, append to query string /static/htmx.js?v={hash} when calling them in script/link
// // #[derive(StaticFiles)]
// // #[folder = "static"]
// // enum StaticFile {
// //     #[name("tailwind.css")]
// //     Tailwind,
// // }
