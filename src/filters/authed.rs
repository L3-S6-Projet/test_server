use crate::db::Database;
use crate::db::{models::UserKind, Db};
use crate::filters::with_db;

use warp::{Filter, Rejection};

/// Filter that checks if the user is authenticated or not, and rejects the request if he/she isn't
pub fn authed(db: &Db) -> impl Filter<Extract = (String,), Error = Rejection> + Clone {
    with_db(db.clone())
        .and(warp::header::optional::<String>("Authorization"))
        .and_then(guard)
}

#[derive(Eq, PartialEq)]
pub enum PossibleUserKind {
    Administrator,
    Teacher,
    Student,
}

/// Filters that checks if the user is of the requested kind, and rejects the request if he/she doesn't
/// have the authorization ; also checks if the user is authenticated.
pub fn authed_is_of_kind<'a>(
    db: &Db,
    role: &'a [PossibleUserKind],
) -> impl Filter<Extract = (String,), Error = Rejection> + Clone + 'a {
    with_db(db.clone())
        .and(authed(db))
        .map(move |db, username| (db, username, role))
        .untuple_one()
        .and_then(guard_kind)
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
        (parts.next().unwrap_or(""), parts.next().unwrap_or(""))
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

async fn guard_kind(
    db: Db,
    username: String,
    wanted_kind: &[PossibleUserKind],
) -> Result<String, warp::Rejection> {
    let db = db.lock().await;
    let user = db
        .user_get(&username)
        .expect("user should be authenticated already");

    let kind = match user.kind {
        UserKind::Administrator => PossibleUserKind::Administrator,
        UserKind::Teacher(_) => PossibleUserKind::Teacher,
        UserKind::Student(_) => PossibleUserKind::Student,
    };

    if wanted_kind.contains(&kind) {
        Ok(username)
    } else {
        Err(warp::reject::custom(Unauthorized {}))
    }
}
