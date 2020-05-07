use serde::{Deserialize, Serialize};
use warp::{Filter, Rejection, Reply};

use super::globals::SimpleSuccessResponse;
use db::{models::UserKind, Database, Db};
use filters::{delayed, with_db, Forbidden};

#[derive(Deserialize)]
struct LoginRequest {
    username: String,
    password: String,
}

#[derive(Serialize)]
struct LoginResponse<'a> {
    status: &'a str,
    token: &'a str,
    user: LoginResponseUser<'a>,
}

#[derive(Serialize)]
struct LoginResponseUser<'a> {
    id: u32,
    first_name: &'a str,
    last_name: &'a str,
    kind: &'a str,
}

pub fn routes(db: &Db) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    let post_session_route = warp::path!("api" / "session")
        .and(warp::post())
        .and(warp::body::content_length_limit(1024 * 16).and(warp::body::json()))
        .and(with_db(db.clone()))
        .and_then(post_session)
        .and(delayed(db))
        .boxed();

    let delete_session_route = warp::path!("api" / "session")
        .and(warp::delete())
        .and(warp::header::<String>("Authorization"))
        .and(with_db(db.clone()))
        .and_then(delete_session)
        .and(delayed(db))
        .boxed();

    post_session_route.or(delete_session_route)
}

async fn post_session(request: LoginRequest, db: Db) -> Result<impl warp::Reply, warp::Rejection> {
    let mut db = db.lock().await;

    match db.auth_login(&request.username, &request.password) {
        Some((user, token)) => Ok(warp::reply::json(&LoginResponse {
            status: "success",
            token: &token,
            user: LoginResponseUser {
                id: user.id,
                first_name: &user.first_name,
                last_name: &user.last_name,
                kind: match user.kind {
                    UserKind::Administrator => "ADM",
                    UserKind::Teacher(_) => "TEA",
                    UserKind::Student(_) => "STU",
                },
            },
        })),
        None => Err(warp::reject::custom(Forbidden {})),
    }
}

async fn delete_session(
    authorization: String,
    db: Db,
) -> Result<impl warp::Reply, warp::Rejection> {
    let mut db = db.lock().await;

    let (auth_type, token) = {
        let mut parts = authorization.splitn(2, " ");
        (parts.next().unwrap_or(""), parts.next().unwrap_or(""))
    };

    let logged_out = auth_type.to_ascii_lowercase() == "bearer" && db.auth_logout(&token);

    if logged_out {
        Ok(warp::reply::json(&SimpleSuccessResponse::new()))
    } else {
        Err(warp::reject::custom(Forbidden {}))
    }
}
