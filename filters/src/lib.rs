mod authed;
mod delayed;
mod with_db;

pub use authed::{authed, authed_is_of_kind, Forbidden, PossibleUserKind, Unauthorized};
pub use delayed::delayed;
pub use with_db::with_db;

#[derive(Debug)]
pub struct Malformed;

impl warp::reject::Reject for Malformed {}
