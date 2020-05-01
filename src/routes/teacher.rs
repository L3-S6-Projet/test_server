use rand::{self, distributions::Alphanumeric, Rng};
use serde::{Deserialize, Serialize};
use warp::{http::StatusCode, Filter, Rejection, Reply};

use super::{
    globals::{
        deserialize_some, AccountCreatedResponse, PaginatedQueryableListRequest,
        SimpleSuccessResponse,
    },
    ErrorCode, FailureResponse,
};
use db::{
    models::{Rank, TeacherInformations, UserKind},
    Database, Db, NewUser,
};
use filters::{authed_is_of_kind, delayed, with_db, PossibleUserKind};

pub fn routes(db: &Db) -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone {
    let list_route = warp::path!("api" / "teachers")
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
    let create_route = warp::path!("api" / "teachers")
        .and(warp::post())
        .and(authed_is_of_kind(db, &[PossibleUserKind::Administrator]))
        .and(with_db(db.clone()))
        .and(warp::body::content_length_limit(1024 * 16).and(warp::body::json()))
        .and_then(create)
        .and(delayed(db))
        .boxed();

    // TODO: deletion constraints
    let delete_route = warp::path!("api" / "teachers")
        .and(warp::delete())
        .and(authed_is_of_kind(db, &[PossibleUserKind::Administrator]))
        .and(with_db(db.clone()))
        .and(warp::body::content_length_limit(1024 * 16).and(warp::body::json()))
        .and_then(delete)
        .and(delayed(db))
        .boxed();

    let get_route = warp::path!("api" / "teachers" / u32)
        .and(warp::get())
        .and(with_db(db.clone()))
        .and_then(get)
        .and(delayed(db))
        .boxed();

    let update_route = warp::path!("api" / "teachers" / u32)
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
    teachers: Vec<Teacher<'a>>,
}

#[derive(Serialize)]
struct Teacher<'a> {
    id: u32,
    first_name: &'a str,
    last_name: &'a str,
    email: Option<&'a str>,
    phone_number: Option<&'a str>,
}

async fn list(
    _username: String,
    db: Db,
    request: PaginatedQueryableListRequest,
) -> Result<impl warp::Reply, warp::Rejection> {
    let db = db.lock().await;

    let page = request.normalized_page_number();
    let (total, users) = db.user_list(page, request.query.as_deref(), |u| match u.kind {
        UserKind::Student(_) => false,
        UserKind::Administrator => false,
        UserKind::Teacher(_) => true,
    });

    let teachers = users
        .into_iter()
        .map(|u| match &u.kind {
            UserKind::Teacher(informations) => Teacher {
                id: u.id,
                first_name: &u.first_name,
                last_name: &u.last_name,
                email: informations.email.as_deref(),
                phone_number: informations.phone_number.as_deref(),
            },
            UserKind::Administrator => unreachable!(),
            UserKind::Student(_) => unreachable!(),
        })
        .collect();

    Ok(warp::reply::json(&ListResponse {
        status: "success",
        total,
        teachers,
    }))
}

#[derive(Deserialize)]
struct NewTeacher {
    first_name: String,
    last_name: String,
    email: Option<String>,
    phone_number: Option<String>,
    rank: Rank,
}

async fn create(
    _username: String,
    db: Db,
    request: NewTeacher,
) -> Result<impl warp::Reply, warp::Rejection> {
    let mut db = db.lock().await;

    let mut rng = rand::thread_rng();

    let password = std::iter::repeat(())
        .map(|()| rng.sample(Alphanumeric))
        .take(10)
        .collect();

    let user = NewUser {
        first_name: request.first_name,
        last_name: request.last_name,
        password,
        kind: UserKind::Teacher(TeacherInformations {
            phone_number: request.phone_number,
            email: request.email,
            rank: request.rank,
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

    let all_exist_and_teacher =
        request
            .iter()
            .map(|id| db.user_get_by_id(*id))
            .all(|user| match user.map(|u| &u.kind) {
                Some(UserKind::Teacher(_)) => true,
                Some(UserKind::Administrator) | Some(UserKind::Student(_)) | None => false,
            });

    if !all_exist_and_teacher {
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
    teacher: GetResponseTeacher<'a>,
}

#[derive(Serialize)]
struct GetResponseTeacher<'a> {
    first_name: &'a str,
    last_name: &'a str,
    username: &'a str,
    email: Option<&'a str>,
    phone_number: Option<&'a str>,
    rank: &'a Rank,
    //total_service: u32, // TODO: total_service
    // TODO: services
}

async fn get(id: u32, db: Db) -> Result<impl warp::Reply, std::convert::Infallible> {
    let db = db.lock().await;
    let user = db.user_get_by_id(id);

    let res_teacher = match user {
        Some(user) => match &user.kind {
            UserKind::Administrator => None,
            UserKind::Teacher(informations) => Some(GetResponseTeacher {
                first_name: &user.first_name,
                last_name: &user.last_name,
                username: &user.username,
                email: informations.email.as_deref(),
                phone_number: informations.phone_number.as_deref(),
                rank: &informations.rank,
            }),
            UserKind::Student(_) => None,
        },
        None => None,
    };

    match res_teacher {
        Some(res_teacher) => Ok(warp::reply::with_status(
            warp::reply::json(&GetResponse {
                status: "success",
                teacher: res_teacher,
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
struct TeacherUpdate {
    first_name: Option<String>,
    last_name: Option<String>,
    #[serde(default, deserialize_with = "deserialize_some")]
    email: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_some")]
    phone_number: Option<Option<String>>,
    rank: Option<Rank>,
    password: Option<String>,
}

async fn update(
    id: u32,
    db: Db,
    request: TeacherUpdate,
) -> Result<impl warp::Reply, std::convert::Infallible> {
    let mut db = db.lock().await;

    let mut user = match db.user_get_teacher_by_id(id) {
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

    macro_rules! update {
        ($obj:ident, $property:ident) => {
            if let Some(value) = request.$property {
                $obj.$property = value;
                updated = true;
            }
        };
    }

    update!(user, first_name);
    update!(user, last_name);
    update!(user, password);

    let mut informations = match &mut user.kind {
        UserKind::Administrator => unreachable!(),
        UserKind::Teacher(infos) => infos,
        UserKind::Student(_) => unreachable!(),
    };

    update!(informations, email);
    update!(informations, phone_number);
    update!(informations, rank);

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
