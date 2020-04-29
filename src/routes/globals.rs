use serde::Serialize;

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
