use futures_util::StreamExt;
use rand::RngCore;
use sha2::{Digest, Sha256};
use tokio::time;
use tokio::time::Duration;
use tokio_stream::wrappers::UnboundedReceiverStream;
use uuid::Uuid;
use warp::{sse, Filter, Rejection, Reply};

pub use error::*;
use goat_api::{Action, GameId, GoatError, UserId};
use goat_bot::{AdaptSimple, Bot, CoverSimple, DuckSimple, PlayTopSimple, Strategy};
pub use server::*;
pub use subscriber::*;

mod error;
mod server;
mod subscriber;

#[cfg(test)]
mod test;

fn user_id() -> impl Filter<Extract = (UserId,), Error = Rejection> + Clone {
    warp::cookie("USER_ID").map(|id: String| {
        let hash = Sha256::digest(id.as_bytes());
        UserId(Uuid::from_slice(&hash[..16]).unwrap())
    })
}

fn user_name() -> impl Filter<Extract = (String,), Error = Rejection> + Clone {
    warp::cookie("USER_NAME")
}

fn assets() -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    warp::path("assets")
        .and(warp::get())
        .and(warp::fs::dir("./assets"))
}

fn new_game(
    state: &'static Server,
) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    fn handle(state: &Server) -> impl Reply {
        let seed = rand::thread_rng().next_u64();
        let game_id = state.new_game(seed);
        warp::reply::json(&game_id)
    }
    warp::path!("new_game")
        .and(warp::post())
        .and(warp::any().map(move || state))
        .map(handle)
}

fn change_name(
    state: &'static Server,
) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    fn handle(state: &Server, user_id: UserId, user_name: String) -> impl Reply {
        state.change_name(user_id, user_name);
        warp::reply()
    }
    warp::path!("change_name")
        .and(warp::post())
        .and(warp::any().map(move || state))
        .and(user_id())
        .and(user_name())
        .map(handle)
}

fn apply_action(
    state: &'static Server,
) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    async fn handle(
        state: &Server,
        user_id: UserId,
        game_id: GameId,
        action: Action,
    ) -> Result<impl Reply, Rejection> {
        state
            .apply_action(user_id, game_id, action)
            .map_err(|e| Error::from(e))?;
        Ok(warp::reply())
    }
    warp::path!("apply_action")
        .and(warp::post())
        .and(warp::any().map(move || state))
        .and(user_id())
        .and(warp::query())
        .and(warp::body::json())
        .and_then(handle)
}

fn subscribe(
    state: &'static Server,
) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    fn handle(state: &Server, user_id: UserId, user_name: String) -> impl Reply {
        let rx = state.subscribe(user_id, user_name);
        let rx = UnboundedReceiverStream::new(rx);
        let stream = rx
            .map(|response| Ok::<_, GoatError>(sse::Event::default().json_data(response).unwrap()));
        sse::reply(stream)
    }
    warp::path!("subscribe")
        .and(warp::get())
        .and(warp::any().map(move || state))
        .and(user_id())
        .and(user_name())
        .map(handle)
}

fn run_bot<S: Strategy + Send + 'static>(state: &'static Server, name: String, strategy: S) {
    tokio::spawn(async move {
        let user_id = UserId(Uuid::new_v4());
        let rx = state.subscribe(user_id, name);
        let tx = |user_id, game_id, action| state.apply_action(user_id, game_id, action);
        let mut bot = Bot::new(user_id, rx, tx, strategy);
        if let Err(e) = bot.run().await {
            log::error!("Bot {} failed: {}", user_id, e);
        }
    });
}

#[tokio::main]
async fn main() {
    env_logger::init();
    let state = &*Box::leak(Box::new(Server::new()));

    tokio::spawn(async move {
        let mut ticker = time::interval(Duration::from_secs(20));
        loop {
            ticker.tick().await;
            state.ping_subscribers();
        }
    });

    tokio::spawn(async move {
        let mut ticker = time::interval(Duration::from_secs(10 * 60));
        loop {
            ticker.tick().await;
            state.drop_old_games();
        }
    });

    run_bot(state, "Alice (bot)".to_string(), PlayTopSimple);
    run_bot(state, "Bob (bot)".to_string(), AdaptSimple);
    run_bot(state, "Carla (bot)".to_string(), CoverSimple);
    run_bot(state, "Dimitri (bot)".to_string(), DuckSimple);
    run_bot(state, "Eric (bot)".to_string(), PlayTopSimple);
    run_bot(state, "Felicia (bot)".to_string(), AdaptSimple);
    run_bot(state, "George (bot)".to_string(), CoverSimple);
    run_bot(state, "Hannah (bot)".to_string(), DuckSimple);

    let app = assets()
        .or(new_game(state))
        .or(change_name(state))
        .or(apply_action(state))
        .or(subscribe(state))
        .recover(handle_error)
        .with(warp::log("request"));
    warp::serve(app).run(([127, 0, 0, 1], 9402)).await;
}
