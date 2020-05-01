use db::Database;
use db::Db;
use crate::with_db;
use warp::{Filter, Rejection};

pub fn delayed(db: &Db) -> impl Filter<Extract = (), Error = Rejection> + Clone {
    warp::any()
        .and(with_db(db.clone()))
        .and_then(delay)
        .untuple_one()
}

async fn delay(db: Db) -> Result<(), warp::Rejection> {
    // Release db as soon as possible
    let delay = {
        let db = db.lock().await;
        db.delay_get()
    };
    tokio::time::delay_for(delay).await;
    Ok(())
}
