use db::{ConcreteDb, Database, Db};
use filters::with_db;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use warp::{Filter, Rejection, Reply};

pub fn routes(db: &Db) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    let index_route = warp::get().and(warp::path::end()).and_then(index);

    let dump_route = warp::path!("api" / "dump")
        .and(warp::get())
        .and(with_db(db.clone()))
        .and_then(dump)
        .boxed();

    let reset_route = warp::path!("api" / "reset")
        .and(warp::get())
        .and(with_db(db.clone()))
        .and_then(reset)
        .boxed();

    let delay_route = warp::path!("api" / "delay")
        .and(warp::get())
        .and(with_db(db.clone()))
        .and_then(delay)
        .boxed();

    let set_delay_route = warp::path!("api" / "delay" / u64)
        .and(warp::get())
        .and(with_db(db.clone()))
        .and_then(set_delay)
        .boxed();

    let swagger_route = warp::path!("swagger.json")
        .and(warp::get())
        .and_then(swagger)
        .boxed();

    let swagger_ui_route = warp::path!("swagger").and(warp::get()).and_then(swagger_ui);

    let export_route = warp::path!("api" / "export")
        .and(warp::get())
        .and(with_db(db.clone()))
        .and_then(export)
        .boxed();

    let import_route = warp::path!("api" / "import")
        .and(warp::get())
        .and(with_db(db.clone()))
        .and_then(import)
        .boxed();

    index_route
        .or(dump_route)
        .or(reset_route)
        .or(delay_route)
        .or(set_delay_route)
        .or(swagger_route)
        .or(swagger_ui_route)
        .or(export_route)
        .or(import_route)
}

async fn index() -> Result<impl warp::Reply, warp::Rejection> {
    // TODO: use const fn once replace is stabilized
    let html =
        include_str!("../../assets/panel.html").replace("#VERSION", env!("CARGO_PKG_VERSION"));

    Ok(warp::reply::html(html))
}

async fn swagger() -> Result<impl warp::Reply, warp::Rejection> {
    Ok(warp::reply::with_header(
        include_str!("../../assets/swagger.json"),
        "content-type",
        "application/json",
    ))
}

async fn swagger_ui() -> Result<impl warp::Reply, warp::Rejection> {
    Ok(warp::reply::html(include_str!(
        "../../assets/swagger_ui.html"
    )))
}

async fn dump(db: Db) -> Result<impl warp::Reply, warp::Rejection> {
    let db = db.lock().await;

    Ok(warp::reply::with_header(
        db.dump_as_json().unwrap(),
        "content-type",
        "application/json",
    ))
}

// Resets the database
async fn reset(db: Db) -> Result<impl warp::Reply, std::convert::Infallible> {
    let mut db = db.lock().await;
    db.reset();
    Ok(warp::reply::json(&"ok".to_string()))
}

async fn delay(db: Db) -> Result<impl warp::Reply, std::convert::Infallible> {
    let db = db.lock().await;
    let delay = db.delay_get().as_millis();
    Ok(warp::reply::json(&delay))
}

async fn set_delay(delay: u64, db: Db) -> Result<impl warp::Reply, std::convert::Infallible> {
    let mut db = db.lock().await;
    db.delay_set(Duration::from_millis(delay));
    Ok(warp::reply::json(&"ok".to_string()))
}

async fn export(db: Db) -> Result<impl warp::Reply, warp::Rejection> {
    let db = db.lock().await;

    let dump = db.dump_as_json().expect("could not dump");

    let mut output = tokio::fs::File::create("save.json")
        .await
        .expect("could not create DB");

    output
        .write_all(dump.as_bytes())
        .await
        .expect("could not persist DB");

    Ok(warp::reply::json(&"ok".to_string()))
}

async fn import(db: Db) -> Result<impl warp::Reply, warp::Rejection> {
    let mut db = db.lock().await;
    let new_db = match ConcreteDb::from_file("save.json") {
        Ok(db) => db,
        Err(_) => return Ok(warp::reply::json(&"failed to read file".to_string())),
    };
    *db = new_db;
    Ok(warp::reply::json(&"ok".to_string()))
}
