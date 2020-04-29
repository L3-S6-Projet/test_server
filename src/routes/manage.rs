use crate::{
    db::{Database, Db},
    filters::with_db,
};
use std::time::Duration;
use warp::{Filter, Rejection, Reply};

pub fn routes(db: &Db) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    let index_route = warp::get().and(warp::path::end()).and_then(index);

    let dump_route = warp::path!("api" / "dump")
        .and(warp::get())
        .and(with_db(db.clone()))
        .and_then(dump);

    let reset_route = warp::path!("api" / "reset")
        .and(warp::get())
        .and(with_db(db.clone()))
        .and_then(reset);

    let delay_route = warp::path!("api" / "delay")
        .and(warp::get())
        .and(with_db(db.clone()))
        .and_then(delay);

    let set_delay_route = warp::path!("api" / "delay" / u64)
        .and(warp::get())
        .and(with_db(db.clone()))
        .and_then(set_delay);

    index_route
        .or(dump_route)
        .or(reset_route)
        .or(delay_route)
        .or(set_delay_route)
}

async fn index() -> Result<impl warp::Reply, warp::Rejection> {
    Ok(warp::reply::html(include_str!("../../assets/panel.html")))
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
