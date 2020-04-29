use crate::db::Database;
use crate::db::Db;
use crate::filters::with_db;

use warp::{Filter, Rejection};

/// Filter that checks if the user is authenticated or not, and rejects the request if he/she isn't
pub fn authed(db: &Db) -> impl Filter<Extract = (String,), Error = Rejection> + Clone {
    with_db(db.clone())
        .and(warp::header::optional::<String>("Authorization"))
        .and_then(guard)
}

#[derive(Debug)]
pub struct Forbidden;

impl warp::reject::Reject for Forbidden {}

#[derive(Debug)]
pub struct Unauthorized;

impl warp::reject::Reject for Unauthorized {}

async fn guard(db: Db, authorization: Option<String>) -> Result<String, warp::Rejection> {
    let authorization = match authorization {
        Some(authorization) => authorization,
        None => return Err(warp::reject::custom(Forbidden {})),
    };

    let (auth_type, token) = {
        let mut parts = authorization.splitn(2, " ");
        (parts.next().unwrap(), parts.next().unwrap())
    };

    if auth_type.to_ascii_lowercase() == "bearer" {
        let db = db.lock().await;

        match db.auth_get_user(&token) {
            Some(user) => Ok(user.username.clone()), // TODO: remove extra allocation + remove extra DB lock
            None => Err(warp::reject::custom(Forbidden {})), // TODO: return proper 401
        }
    } else {
        Err(warp::reject::custom(Forbidden {}))
    }
}
