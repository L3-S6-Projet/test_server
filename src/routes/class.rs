use serde::Serialize;
use warp::{http::StatusCode, Filter, Rejection, Reply};

use super::{
    globals::{
        OccupanciesListResponse, OccupanciesRequest, PaginatedQueryableListRequest,
        SimpleSuccessResponse,
    },
    ErrorCode, FailureResponse,
};
use crate::service::service_value;
use db::{
    models::{Class, ClassLevel, Occupancy},
    ClassUpdate, Database, Db, NewClass,
};
use filters::{authed_is_of_kind, delayed, with_db, PossibleUserKind};

pub fn routes(db: &Db) -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone {
    let list_route = warp::path!("api" / "classes")
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
    let create_route = warp::path!("api" / "classes")
        .and(warp::post())
        .and(authed_is_of_kind(db, &[PossibleUserKind::Administrator]))
        .and(with_db(db.clone()))
        .and(warp::body::content_length_limit(1024 * 16).and(warp::body::json()))
        .and_then(create)
        .and(delayed(db))
        .boxed();

    // TODO: deletion constraints
    let delete_route = warp::path!("api" / "classes")
        .and(warp::delete())
        .and(authed_is_of_kind(db, &[PossibleUserKind::Administrator]))
        .and(with_db(db.clone()))
        .and(warp::body::content_length_limit(1024 * 16).and(warp::body::json()))
        .and_then(delete)
        .and(delayed(db))
        .boxed();

    let get_route = warp::path!("api" / "classes" / u32)
        .and(warp::get())
        .and(with_db(db.clone()))
        .and_then(get)
        .and(delayed(db))
        .boxed();

    let update_route = warp::path!("api" / "classes" / u32)
        .and(warp::put())
        .and(with_db(db.clone()))
        .and(warp::body::content_length_limit(1024 * 16).and(warp::body::json()))
        .and_then(update)
        .and(delayed(db))
        .boxed();

    let occupancies_get_route = warp::path!("api" / "classes" / u32 / "occupancies")
        .and(warp::get())
        .and(with_db(db.clone()))
        .and(warp::query::<OccupanciesRequest>())
        .and_then(occupancies_get)
        .and(delayed(db))
        .boxed();

    list_route
        .or(create_route)
        .or(delete_route)
        .or(get_route)
        .or(update_route)
        .or(occupancies_get_route)
}

#[derive(Serialize)]
struct ListResponse<'a> {
    status: &'static str,
    total: usize,
    classes: Vec<&'a Class>,
}

async fn list(
    _username: String,
    db: Db,
    request: PaginatedQueryableListRequest,
) -> Result<impl warp::Reply, warp::Rejection> {
    let db = db.lock().await;

    let page = request.normalized_page_number();
    let (total, classes) = db.class_list(page, request.query.as_deref());

    Ok(warp::reply::json(&ListResponse {
        status: "success",
        total,
        classes,
    }))
}

async fn create(
    _username: String,
    db: Db,
    request: NewClass,
) -> Result<impl warp::Reply, warp::Rejection> {
    let mut db = db.lock().await;
    db.class_add(request);
    Ok(warp::reply::json(&SimpleSuccessResponse::new()))
}

async fn delete(
    _username: String,
    db: Db,
    request: Vec<u32>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let mut db = db.lock().await;

    if db.class_remove(&request) {
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
    class: GetResponseClass<'a>,
    total_service: u32,
}

#[derive(Serialize)]
struct GetResponseClass<'a> {
    name: &'a str,
    level: &'a ClassLevel,
}

async fn get(id: u32, db: Db) -> Result<impl warp::Reply, std::convert::Infallible> {
    let db = db.lock().await;
    let class = db.class_get(id);

    match class {
        Some(class) => {
            // Total service: somme de tous les cours
            let occupancies_list = db.occupancies_list(None, None);

            let occupancies_list: Vec<&Occupancy> = occupancies_list
                .into_iter()
                .filter(|o| {
                    let subject_id = match o.subject_id {
                        Some(i) => i,
                        None => return false,
                    };

                    let subject = match db.subject_get(subject_id) {
                        Some(s) => s,
                        None => return false,
                    };

                    subject.class_id == id
                })
                .collect();

            let total_service = service_value(occupancies_list.as_slice()) as u32;

            Ok(warp::reply::with_status(
                warp::reply::json(&GetResponse {
                    status: "success",
                    class: GetResponseClass {
                        name: &class.name,
                        level: &class.level,
                    },
                    total_service,
                }),
                StatusCode::OK,
            ))
        }
        None => Ok(warp::reply::with_status(
            warp::reply::json(&FailureResponse::new(ErrorCode::InvalidID)),
            StatusCode::NOT_FOUND,
        )),
    }
}

async fn update(
    id: u32,
    db: Db,
    request: ClassUpdate,
) -> Result<impl warp::Reply, std::convert::Infallible> {
    let mut db = db.lock().await;
    let status = db.class_update(id, request);

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

async fn occupancies_get(
    id: u32,
    db: Db,
    request: OccupanciesRequest,
) -> Result<impl warp::Reply, warp::Rejection> {
    let db = db.lock().await;

    if db.class_get(id).is_none() {
        return Ok(warp::reply::with_status(
            warp::reply::json(&FailureResponse::new(ErrorCode::InvalidID)),
            StatusCode::NOT_FOUND,
        ));
    }

    let occupancies_list = db.occupancies_list(request.start, request.end);

    let occupancies_list = occupancies_list
        .into_iter()
        .filter(|o| match o.subject_id {
            Some(subject_id) => {
                let subject = db
                    .subject_get(subject_id)
                    .expect("should be a valid reference");

                let class = db
                    .class_get(subject.class_id)
                    .expect("should be a valid reference");

                class.id == id
            }
            None => false,
        })
        .collect();

    let response =
        OccupanciesListResponse::from_list(&db, occupancies_list, request.occupancies_per_day);

    Ok(warp::reply::with_status(
        warp::reply::json(&response),
        StatusCode::OK,
    ))
}
