use super::globals::{OccupanciesListResponse, OccupanciesRequest};
use db::{Database, Db, LockedDb};
use filters::{authed_is_of_kind, delayed, with_db, PossibleUserKind};
use warp::{Filter, Rejection, Reply};

pub fn routes(db: &Db) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    let get_route = warp::path!("api" / "occupancies")
        .and(warp::get())
        .and(authed_is_of_kind(db, &[PossibleUserKind::Administrator]))
        .and(with_db(db.clone()))
        .and(warp::query::<OccupanciesRequest>())
        .and_then(get)
        .and(delayed(db))
        .boxed();

    get_route
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
