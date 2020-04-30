use serde::Deserialize;
use warp::{http::StatusCode, Filter, Rejection, Reply};

use super::globals::{ErrorCode, FailureResponse, SimpleSuccessResponse};
use crate::db::Database;
use crate::db::Db;
use crate::filters::{authed, delayed, with_db, Malformed, Unauthorized};

pub fn routes(db: &Db) -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone {
    let put_profile_route = warp::path!("api" / "profile")
        .and(warp::put())
        .and(authed(db))
        .and(warp::body::content_length_limit(1024 * 16).and(warp::body::json()))
        .and(with_db(db.clone()))
        .and_then(put_profile)
        .and(delayed(db));

    put_profile_route
}

#[derive(Deserialize)]
struct UpdateRequest {
    old_password: Option<String>,
    password: Option<String>,
    first_name: Option<String>,
    last_name: Option<String>,
}

async fn put_profile(
    username: String,
    request: UpdateRequest,
    db: Db,
) -> Result<impl warp::Reply, warp::Rejection> {
    let mut db = db.lock().await;

    let mut user = db
        .user_get(&username)
        .expect("checked username should be valid")
        .clone();

    // Check for permissions : only admin users should be able to edit their first and last name.
    if !user.kind.is_administrator()
        && (request.first_name.is_some() || request.last_name.is_some())
    {
        return Err(warp::reject::custom(Unauthorized {}));
    }

    let mut modified = false;

    match (request.old_password, request.password) {
        (Some(old_password), Some(password)) => {
            if user.password != old_password {
                return Ok(warp::reply::with_status(
                    FailureResponse::new_reply(ErrorCode::InvalidOldPassword),
                    StatusCode::FORBIDDEN,
                ));
            }

            user.password = password;
            modified = true;
        }
        // Check for provided password without old_password (or the inverse)
        (None, Some(_)) | (Some(_), None) => {
            return Err(warp::reject::custom(Malformed {}));
        }
        _ => {}
    }

    if let Some(first_name) = request.first_name {
        user.first_name = first_name;
        modified = true;
    }

    if let Some(last_name) = request.last_name {
        user.last_name = last_name;
        modified = true;
    }

    if modified {
        db.user_update(user);
    }

    // Return a 204 if the content didn't change
    let status_code = if modified {
        StatusCode::OK
    } else {
        StatusCode::NO_CONTENT
    };

    Ok(warp::reply::with_status(
        warp::reply::json(&SimpleSuccessResponse::new()),
        status_code,
    ))
}
