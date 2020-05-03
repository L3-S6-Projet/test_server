use warp::{Filter, Rejection, Reply};

use db::Db;

mod auth;
mod class;
mod classroom;
mod globals;
mod manage;
mod occupancy;
mod profile;
mod student;
mod subject;
mod teacher;

pub use globals::{ErrorCode, FailureResponse};

pub fn routes(db: &Db) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    manage::routes(db)
        .or(auth::routes(db))
        .or(profile::routes(db))
        .or(classroom::routes(db))
        .or(class::routes(db))
        .or(teacher::routes(db))
        .or(student::routes(db))
        .or(subject::routes(db))
        .or(occupancy::routes(db))
}
