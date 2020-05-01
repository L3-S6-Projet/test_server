use db::Db;
use std::convert::Infallible;
use warp::Filter;

/// Simple filter to add the database to the request
pub fn with_db(db: Db) -> impl Filter<Extract = (Db,), Error = Infallible> + Clone {
    warp::any().map(move || db.clone())
}
