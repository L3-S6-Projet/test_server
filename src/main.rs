//#![type_length_limit = "1112968"]

// TODO: validate incoming data

use fern::colors::{Color, ColoredLevelConfig};
use warp::{http::StatusCode, Filter, Rejection, Reply};

mod routes;

use db::new_db;
use filters::{Forbidden, Malformed, Unauthorized};
use routes::{routes, ErrorCode, FailureResponse};

#[tokio::main]
async fn main() {
    setup_logging();

    let global_db = new_db("db.json".to_string());
    let filters = routes(&global_db);

    let cors = warp::cors()
        .allow_any_origin()
        .allow_methods(vec!["GET", "POST", "PUT", "DELETE"])
        .allow_headers(vec!["content-type", "authorization"]);

    let filters = filters
        // Before logging for correct status codes, before CORS for proper headers
        .recover(handle_rejection)
        .with(warp::log("dummy"))
        .with(cors);

    warp::serve(filters).run(([0, 0, 0, 0], 3030)).await;
}

fn setup_logging() {
    let colors = ColoredLevelConfig::new().debug(Color::Magenta);

    fern::Dispatch::new()
        .format(move |out, message, record| {
            out.finish(format_args!(
                "{}{} {}",
                colors.color(record.level()),
                chrono::Local::now().format("[%H:%M:%S]"),
                message
            ))
        })
        .level(log::LevelFilter::Info)
        .chain(std::io::stdout())
        .apply()
        .expect("Could not apply logging configuration");
}

async fn handle_rejection(err: Rejection) -> Result<impl Reply, warp::Rejection> {
    let error_code;
    let status_code;

    if err.is_not_found() {
        error_code = ErrorCode::NotFound;
        status_code = StatusCode::NOT_FOUND;
    } else if let Some(Forbidden) = err.find() {
        error_code = ErrorCode::InvalidCredentials;
        status_code = StatusCode::FORBIDDEN;
    } else if let Some(Unauthorized) = err.find() {
        error_code = ErrorCode::InsufficientAuthorization;
        status_code = StatusCode::UNAUTHORIZED;
    } else if let Some(Malformed) = err.find() {
        error_code = ErrorCode::MalformedData;
        status_code = StatusCode::BAD_REQUEST;
    } else {
        // Unknown error : pass it along, will be handled by warp.
        return Err(err);
    }

    let json = warp::reply::json(&FailureResponse::new(error_code));
    Ok(warp::reply::with_status(json, status_code))
}
