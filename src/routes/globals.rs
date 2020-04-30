use serde::{Deserialize, Serialize};

#[derive(Serialize)]
pub struct FailureResponse {
    status: &'static str,
    code: ErrorCode,
}

impl FailureResponse {
    pub fn new(code: ErrorCode) -> Self {
        Self {
            status: "error",
            code,
        }
    }

    pub fn new_reply(code: ErrorCode) -> warp::reply::Json {
        warp::reply::json(&Self::new(code))
    }
}

#[derive(Serialize)]
pub enum ErrorCode {
    InternalServerError,
    NotFound,
    MethodNotAllowed,
    InvalidCredentials,
    InsufficientAuthorization,
    MalformedData,
    InvalidOldPassword,
    InvalidID,
}

#[derive(Serialize)]
pub struct SimpleSuccessResponse {
    status: &'static str,
}

impl SimpleSuccessResponse {
    pub fn new() -> Self {
        Self { status: "success" }
    }
}

#[derive(Deserialize, Debug)]
pub struct PaginatedQueryableListRequest {
    pub query: Option<String>,
    pub page: Option<usize>,
}

impl PaginatedQueryableListRequest {
    /// Checks that the page number is valid, and if its not it returns 1
    pub fn normalized_page_number(&self) -> usize {
        self.page
            .map(|v| if v >= 1 { v } else { 1 })
            .unwrap_or(1usize)
    }
}
