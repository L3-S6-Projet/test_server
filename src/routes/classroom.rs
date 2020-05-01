use serde::Serialize;
use warp::{http::StatusCode, Filter, Rejection, Reply};

use super::{
    globals::{PaginatedQueryableListRequest, SimpleSuccessResponse},
    ErrorCode, FailureResponse,
};
use db::{models::Classroom, ClassroomUpdate, Database, Db, NewClassroom};
use filters::{authed_is_of_kind, delayed, with_db, PossibleUserKind};

pub fn routes(db: &Db) -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone {
    let list_route = warp::path!("api" / "classrooms")
        .and(warp::get())
        .and(authed_is_of_kind(
            db,
            &[PossibleUserKind::Administrator, PossibleUserKind::Teacher],
        ))
        .and(with_db(db.clone()))
        .and(warp::query::<PaginatedQueryableListRequest>())
        .and_then(list)
        .and(delayed(db))
        .boxed();

    // TODO: creation constraint (unique name?)
    let create_route = warp::path!("api" / "classrooms")
        .and(warp::post())
        .and(authed_is_of_kind(db, &[PossibleUserKind::Administrator]))
        .and(with_db(db.clone()))
        .and(warp::body::content_length_limit(1024 * 16).and(warp::body::json()))
        .and_then(create)
        .and(delayed(db))
        .boxed();

    // TODO: deletion constraints
    let delete_route = warp::path!("api" / "classrooms")
        .and(warp::delete())
        .and(authed_is_of_kind(db, &[PossibleUserKind::Administrator]))
        .and(with_db(db.clone()))
        .and(warp::body::content_length_limit(1024 * 16).and(warp::body::json()))
        .and_then(delete)
        .and(delayed(db))
        .boxed();

    let get_route = warp::path!("api" / "classrooms" / u32)
        .and(warp::get())
        .and(with_db(db.clone()))
        .and_then(get)
        .and(delayed(db))
        .boxed();

    let update_route = warp::path!("api" / "classrooms" / u32)
        .and(warp::put())
        .and(with_db(db.clone()))
        .and(warp::body::content_length_limit(1024 * 16).and(warp::body::json()))
        .and_then(update)
        .and(delayed(db))
        .boxed();

    list_route
        .or(create_route)
        .or(delete_route)
        .or(get_route)
        .or(update_route)
}

#[derive(Serialize)]
struct ListResponse<'a> {
    status: &'static str,
    total: usize,
    classrooms: Vec<&'a Classroom>,
}

async fn list(
    _username: String,
    db: Db,
    request: PaginatedQueryableListRequest,
) -> Result<impl warp::Reply, warp::Rejection> {
    let db = db.lock().await;

    let page = request.normalized_page_number();
    let (total, classrooms) = db.classroom_list(page, request.query.as_deref());

    Ok(warp::reply::json(&ListResponse {
        status: "success",
        total,
        classrooms,
    }))
}

async fn create(
    _username: String,
    db: Db,
    request: NewClassroom,
) -> Result<impl warp::Reply, warp::Rejection> {
    let mut db = db.lock().await;
    db.classroom_add(request);
    Ok(warp::reply::json(&SimpleSuccessResponse::new()))
}

async fn delete(
    _username: String,
    db: Db,
    request: Vec<u32>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let mut db = db.lock().await;

    if db.classroom_remove(&request) {
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

#[derive(Serialize)]
struct GetResponse<'a> {
    status: &'static str,
    classroom: &'a Classroom,
}

async fn get(id: u32, db: Db) -> Result<impl warp::Reply, std::convert::Infallible> {
    let db = db.lock().await;
    let classroom = db.classroom_get(id);

    match classroom {
        Some(classroom) => Ok(warp::reply::with_status(
            warp::reply::json(&GetResponse {
                status: "success",
                classroom,
            }),
            StatusCode::OK,
        )),
        None => Ok(warp::reply::with_status(
            warp::reply::json(&FailureResponse::new(ErrorCode::InvalidID)),
            StatusCode::NOT_FOUND,
        )),
    }
}

async fn update(
    id: u32,
    db: Db,
    request: ClassroomUpdate,
) -> Result<impl warp::Reply, std::convert::Infallible> {
    let mut db = db.lock().await;
    let status = db.classroom_update(id, request);

    if status.found {
        Ok(warp::reply::with_status(
            warp::reply::json(&SimpleSuccessResponse::new()),
            if status.updated {
                StatusCode::OK
            } else {
                StatusCode::NO_CONTENT
            },
        ))
    } else {
        Ok(warp::reply::with_status(
            warp::reply::json(&FailureResponse::new(ErrorCode::InvalidID)),
            StatusCode::NOT_FOUND,
        ))
    }
}
