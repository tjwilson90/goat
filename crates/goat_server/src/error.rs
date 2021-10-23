use std::convert::Infallible;

use thiserror::Error;
use warp::http::{Response, StatusCode};
use warp::reject::Reject;
use warp::{Rejection, Reply};

use goat_api::GoatError;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Rules error: {error}")]
    Rules {
        #[from]
        error: GoatError,
    },
}

impl Reject for Error {}

pub async fn handle_error(err: Rejection) -> Result<impl Reply, Infallible> {
    if let Some(err) = err.find::<Error>() {
        let mut resp = Response::new(err.to_string());
        *resp.status_mut() = StatusCode::BAD_REQUEST;
        Ok(resp)
    } else if err.is_not_found() {
        let mut resp = Response::new(String::new());
        *resp.status_mut() = StatusCode::NOT_FOUND;
        Ok(resp)
    } else {
        let mut resp = Response::new(format!("{:?}", err));
        *resp.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
        Ok(resp)
    }
}
