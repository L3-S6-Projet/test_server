use serde::{Deserialize, Serialize};
use warp::{http::StatusCode, Filter, Rejection, Reply};

use super::globals::{ErrorCode, FailureResponse, SimpleSuccessResponse};
use db::Database;
use db::{
    models::{ModificationType, OccupancyType},
    Db,
};
use filters::{authed, delayed, with_db, Malformed, Unauthorized};

pub fn routes(db: &Db) -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone {
    let put_profile_route = warp::path!("api" / "profile")
        .and(warp::put())
        .and(authed(db))
        .and(warp::body::content_length_limit(1024 * 16).and(warp::body::json()))
        .and(with_db(db.clone()))
        .and_then(put_profile)
        .and(delayed(db))
        .boxed();

    let last_occupancies_modifications_route =
        warp::path!("api" / "profile" / "last-occupancies-modifications")
            .and(warp::get())
            .and(authed(db))
            .and(with_db(db.clone()))
            .and_then(last_occupancies_modifications)
            .and(delayed(db))
            .boxed();

    put_profile_route.or(last_occupancies_modifications_route)
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

#[derive(Serialize)]
struct LastOccupanciesModificationsResponse {
    status: &'static str,
    modifications: Vec<ModificationResponse>,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct ModificationResponse {
    pub modification_type: ModificationType,
    pub modification_timestamp: u64,
    pub occupancy: ModificationOccupancyResponse,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct ModificationOccupancyResponse {
    pub subject_name: Option<String>,
    pub class_name: Option<String>,
    pub occupancy_type: OccupancyType,
    pub occupancy_start: u64,
    pub occupancy_end: u64,
    pub previous_occupancy_start: u64,
    pub previous_occupancy_end: u64,
}

async fn last_occupancies_modifications(
    username: String,
    db: Db,
) -> Result<impl warp::Reply, warp::Rejection> {
    let db = db.lock().await;
    let user = db.user_get(&username).expect("should be a valid reference");

    let modifications = db.last_occupancies_modifications(user.id);

    Ok(warp::reply::json(&LastOccupanciesModificationsResponse {
        status: "success",
        modifications: modifications
            .iter()
            .map(|m| {
                let class = m
                    .occupancy
                    .class_id
                    .and_then(|class_id| db.class_get(class_id));

                let subject = m
                    .occupancy
                    .subject_id
                    .and_then(|subject_id| db.subject_get(subject_id));

                ModificationResponse {
                    modification_type: m.modification_type.clone(),
                    modification_timestamp: m.modification_timestamp,
                    occupancy: ModificationOccupancyResponse {
                        subject_name: subject.map(|s| s.name.to_string()),
                        class_name: class.map(|c| c.name.to_string()),
                        occupancy_type: m.occupancy.occupancy_type.clone(),
                        occupancy_start: m.occupancy.occupancy_start,
                        occupancy_end: m.occupancy.occupancy_end,
                        previous_occupancy_start: m.occupancy.previous_occupancy_start,
                        previous_occupancy_end: m.occupancy.previous_occupancy_end,
                    },
                }
            })
            .collect(),
    }))
}
