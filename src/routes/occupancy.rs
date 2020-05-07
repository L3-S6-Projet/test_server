use super::{
    globals::{OccupanciesListResponse, OccupanciesRequest, SimpleSuccessResponse},
    ErrorCode, FailureResponse,
};
use db::{Database, Db, LockedDb};
use filters::{authed_is_of_kind, delayed, with_db, PossibleUserKind};
use warp::{http::StatusCode, Filter, Rejection, Reply};

pub fn routes(db: &Db) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    let get_route = warp::path!("api" / "occupancies")
        .and(warp::get())
        .and(authed_is_of_kind(db, &[PossibleUserKind::Administrator]))
        .and(with_db(db.clone()))
        .and(warp::query::<OccupanciesRequest>())
        .and_then(get)
        .and(delayed(db))
        .boxed();

    // TODO: deletion constraints
    let delete_route = warp::path!("api" / "occupancies")
        .and(warp::delete())
        .and(authed_is_of_kind(db, &[PossibleUserKind::Administrator]))
        .and(with_db(db.clone()))
        .and(warp::body::content_length_limit(1024 * 16).and(warp::body::json()))
        .and_then(delete)
        .and(delayed(db))
        .boxed();

    get_route.or(delete_route)
}

async fn get(
    _username: String,
    db: Db,
    request: OccupanciesRequest,
) -> Result<impl warp::Reply, warp::Rejection> {
    let db: LockedDb = db.lock().await;

    let occupancies_list = db.occupancies_list(request.start, request.end);
    let response =
        OccupanciesListResponse::from_list(&db, occupancies_list, request.occupancies_per_day);

    Ok(warp::reply::json(&response))
}

async fn delete(
    _username: String,
    db: Db,
    request: Vec<u32>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let mut db = db.lock().await;

    if db.occupancies_remove(&request) {
        Ok(warp::reply::with_status(
            warp::reply::json(&SimpleSuccessResponse::new()),
            StatusCode::OK,
        ))
    } else {
        Ok(warp::reply::with_status(
            warp::reply::json(&FailureResponse::new(ErrorCode::InvalidID)),
            StatusCode::NOT_FOUND,
        ))
    }
}
