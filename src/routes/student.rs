use rand::{self, distributions::Alphanumeric, Rng};
use serde::{Deserialize, Serialize};
use warp::{http::StatusCode, Filter, Rejection, Reply};

use super::{
    globals::{AccountCreatedResponse, PaginatedQueryableListRequest, SimpleSuccessResponse},
    ErrorCode, FailureResponse,
};
use db::{
    models::{StudentInformations, UserKind},
    Database, Db, NewUser,
};
use filters::{authed_is_of_kind, delayed, with_db, PossibleUserKind};

pub fn routes(db: &Db) -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone {
    let list_route = warp::path!("api" / "students")
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
    let create_route = warp::path!("api" / "students")
        .and(warp::post())
        .and(authed_is_of_kind(db, &[PossibleUserKind::Administrator]))
        .and(with_db(db.clone()))
        .and(warp::body::content_length_limit(1024 * 16).and(warp::body::json()))
        .and_then(create)
        .and(delayed(db))
        .boxed();

    // TODO: deletion constraints
    let delete_route = warp::path!("api" / "students")
        .and(warp::delete())
        .and(authed_is_of_kind(db, &[PossibleUserKind::Administrator]))
        .and(with_db(db.clone()))
        .and(warp::body::content_length_limit(1024 * 16).and(warp::body::json()))
        .and_then(delete)
        .and(delayed(db))
        .boxed();

    let get_route = warp::path!("api" / "students" / u32)
        .and(warp::get())
        .and(with_db(db.clone()))
        .and_then(get)
        .and(delayed(db))
        .boxed();

    let update_route = warp::path!("api" / "students" / u32)
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
    students: Vec<Student<'a>>,
}

#[derive(Serialize)]
struct Student<'a> {
    id: u32,
    first_name: &'a str,
    last_name: &'a str,
    class_name: &'a str,
}

async fn list(
    _username: String,
    db: Db,
    request: PaginatedQueryableListRequest,
) -> Result<impl warp::Reply, warp::Rejection> {
    let db = db.lock().await;

    let page = request.normalized_page_number();
    let (total, users) = db.user_list(page, request.query.as_deref(), |u| match u.kind {
        UserKind::Student(_) => true,
        UserKind::Administrator => false,
        UserKind::Teacher(_) => false,
    });

    let students = users
        .into_iter()
        .map(|u| match &u.kind {
            UserKind::Student(informations) => {
                let class = db
                    .class_get(informations.class_id)
                    .expect("invalid class_id in user informations");

                Student {
                    id: u.id,
                    first_name: &u.first_name,
                    last_name: &u.last_name,
                    class_name: &class.name,
                }
            }
            UserKind::Administrator => unreachable!(),
            UserKind::Teacher(_) => unreachable!(),
        })
        .collect();

    Ok(warp::reply::json(&ListResponse {
        status: "success",
        total,
        students,
    }))
}

#[derive(Deserialize)]
struct NewStudent {
    first_name: String,
    last_name: String,
    class_id: u32,
}

async fn create(
    _username: String,
    db: Db,
    request: NewStudent,
) -> Result<impl warp::Reply, warp::Rejection> {
    let mut db = db.lock().await;

    let class = db.class_get(request.class_id);

    if class.is_none() {
        return Ok(warp::reply::json(&FailureResponse::new(
            ErrorCode::InvalidID,
        )));
    }

    let mut rng = rand::thread_rng();

    let password = std::iter::repeat(())
        .map(|()| rng.sample(Alphanumeric))
        .take(10)
        .collect();

    let user = NewUser {
        first_name: request.first_name,
        last_name: request.last_name,
        password,
        kind: UserKind::Student(StudentInformations {
            class_id: request.class_id,
        }),
    };

    let user = db.user_add(user);

    Ok(warp::reply::json(&AccountCreatedResponse {
        status: "success",
        username: &user.username,
        password: &user.password,
    }))
}

async fn delete(
    _username: String,
    db: Db,
    request: Vec<u32>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let mut db = db.lock().await;

    let all_exist_and_student =
        request
            .iter()
            .map(|id| db.user_get_by_id(*id))
            .all(|user| match user.map(|u| &u.kind) {
                Some(UserKind::Student(_)) => true,
                Some(UserKind::Administrator) | Some(UserKind::Teacher(_)) | None => false,
            });

    if !all_exist_and_student {
        return Ok(warp::reply::with_status(
            warp::reply::json(&FailureResponse::new(ErrorCode::InvalidID)),
            StatusCode::NOT_FOUND,
        ));
    }

    if db.user_remove(&request) {
        Ok(warp::reply::with_status(
            warp::reply::json(&SimpleSuccessResponse::new()),
            StatusCode::OK,
        ))
    } else {
        unreachable!("Since we checked that the users exist, they should be able to be removed")
    }
}

#[derive(Serialize)]
struct GetResponse<'a> {
    status: &'static str,
    student: GetResponseStudent<'a>,
}

#[derive(Serialize)]
struct GetResponseStudent<'a> {
    first_name: &'a str,
    last_name: &'a str,
    username: &'a str,
    // TODO: total_hours + subjects
}

async fn get(id: u32, db: Db) -> Result<impl warp::Reply, std::convert::Infallible> {
    let db = db.lock().await;
    let user = db.user_get_by_id(id);

    let res_student = match user {
        Some(user) => match &user.kind {
            UserKind::Administrator | UserKind::Teacher(_) => None,
            UserKind::Student(_informations) => Some(GetResponseStudent {
                first_name: &user.first_name,
                last_name: &user.last_name,
                username: &user.username,
            }),
        },
        None => None,
    };

    match res_student {
        Some(res_student) => Ok(warp::reply::with_status(
            warp::reply::json(&GetResponse {
                status: "success",
                student: res_student,
            }),
            StatusCode::OK,
        )),
        None => Ok(warp::reply::with_status(
            warp::reply::json(&FailureResponse::new(ErrorCode::InvalidID)),
            StatusCode::NOT_FOUND,
        )),
    }
}

#[derive(Deserialize, Debug)]
struct StudentUpdate {
    first_name: Option<String>,
    last_name: Option<String>,
    class_id: Option<u32>,
    password: Option<String>,
}

async fn update(
    id: u32,
    db: Db,
    request: StudentUpdate,
) -> Result<impl warp::Reply, std::convert::Infallible> {
    let mut db = db.lock().await;

    let user = db.user_get_by_id(id).and_then(|user| match &user.kind {
        UserKind::Administrator => None,
        UserKind::Teacher(_) => None,
        UserKind::Student(_) => Some(user),
    });

    let mut user = match user {
        Some(user) => user,
        None => {
            return Ok(warp::reply::with_status(
                warp::reply::json(&FailureResponse::new(ErrorCode::InvalidID)),
                StatusCode::NOT_FOUND,
            ))
        }
    }
    .clone();

    let mut updated = false;

    if let Some(value) = request.first_name {
        user.first_name = value;
        updated = true;
    }

    if let Some(value) = request.last_name {
        user.last_name = value;
        updated = true;
    }

    let mut informations = match &mut user.kind {
        UserKind::Administrator => unreachable!(),
        UserKind::Teacher(_) => unreachable!(),
        UserKind::Student(infos) => infos,
    };

    if let Some(class_id) = request.class_id {
        // Check that class exists
        if db.class_get(class_id).is_none() {
            return Ok(warp::reply::with_status(
                warp::reply::json(&FailureResponse::new(ErrorCode::InvalidID)),
                StatusCode::NOT_FOUND,
            ));
        }

        informations.class_id = class_id;
        updated = true;
    }

    if let Some(value) = request.password {
        user.password = value;
        updated = true;
    }

    if updated {
        db.user_update(user);
    }

    Ok(warp::reply::with_status(
        warp::reply::json(&SimpleSuccessResponse::new()),
        if updated {
            StatusCode::OK
        } else {
            StatusCode::NO_CONTENT
        },
    ))
}
