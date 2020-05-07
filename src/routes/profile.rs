use serde::{Deserialize, Serialize};
use warp::{http::StatusCode, Filter, Rejection, Reply};

use super::globals::{ErrorCode, FailureResponse, SimpleSuccessResponse};
use db::Database;
use db::{
    models::{ModificationType, OccupancyType},
    Db,
};
use filters::{authed, delayed, with_db};

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

    let ical_feed_route = warp::path!("api" / "profile" / "feeds" / "ical")
        .and(warp::get())
        .and(authed(db))
        .and_then(ical_feed)
        .and(delayed(db))
        .boxed();

    put_profile_route
        .or(last_occupancies_modifications_route)
        .or(ical_feed_route)
}

#[derive(Deserialize)]
struct UpdateRequest {
    old_password: String,
    password: String,
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

    if user.password != request.old_password {
        return Ok(warp::reply::with_status(
            FailureResponse::new_reply(ErrorCode::InvalidOldPassword),
            StatusCode::FORBIDDEN,
        ));
    }

    user.password = request.password;
    db.user_update(user);

    Ok(warp::reply::with_status(
        warp::reply::json(&SimpleSuccessResponse::new()),
        StatusCode::OK,
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

#[derive(Serialize)]
struct IcalFeedResponse<'a> {
    status: &'static str,
    url: &'a str,
}

async fn ical_feed(_username: String) -> Result<impl warp::Reply, warp::Rejection> {
    Ok(warp::reply::json(&IcalFeedResponse {
        status: "success",
        url: "http://some-fake-url",
    }))
}
