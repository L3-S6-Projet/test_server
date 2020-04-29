use crate::db::Database;
use crate::db::Db;
use crate::filters::{authed, delayed, with_db};
use warp::{Filter, Rejection, Reply};

pub fn routes(db: &Db) -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone {
    let list_route = warp::path!("api" / "classrooms")
        .and(authed(db))
        .and(with_db(db.clone()))
        .and_then(list)
        .and(delayed(db));

    let get_route = warp::path!("api" / "classrooms" / u32)
        .and(with_db(db.clone()))
        .and_then(get)
        .and(delayed(db));

    list_route.or(get_route)
}

async fn list(_username: String, db: Db) -> Result<impl warp::Reply, warp::Rejection> {
    let db = db.lock().await;
    let classrooms = db.classroom_list();
    Ok(warp::reply::json(&classrooms))
}

async fn get(id: u32, db: Db) -> Result<impl warp::Reply, std::convert::Infallible> {
    let db = db.lock().await;
    let classroom = db.classroom_get(id);
    Ok(warp::reply::json(&classroom))
}
