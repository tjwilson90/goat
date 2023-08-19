use futures_util::StreamExt;
use rand::RngCore;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use tokio::time;
use tokio::time::Duration;
use tokio_stream::wrappers::UnboundedReceiverStream;
use warp::{sse, Filter, Rejection, Reply};

pub use error::*;
use goat_api::{Action, GameId, GoatError, RandId, UserId};
use goat_bot::{AdaptSimulate, Bot, Strategy};
pub use server::*;
pub use subscriber::*;

mod error;
mod server;
mod subscriber;

#[cfg(test)]
mod test;

fn user_id() -> impl Filter<Extract = (UserId,), Error = Rejection> + Clone {
    warp::cookie("USER_SECRET").map(|id: String| {
        let hash = Sha256::digest(id.as_bytes());
        UserId(RandId::from_hash(&hash))
    })
}

fn user_name() -> impl Filter<Extract = (String,), Error = Rejection> + Clone {
    warp::cookie("USER_NAME")
}

fn root() -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    warp::path::end()
        .and(warp::get())
        .and(warp::fs::file("./assets/index.html"))
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
    #[derive(Deserialize)]
    struct Wrapper {
        game_id: GameId,
    }
    async fn handle(
        state: &Server,
        user_id: UserId,
        Wrapper { game_id }: Wrapper,
        action: Action,
    ) -> Result<impl Reply, Rejection> {
        state
            .apply_action(user_id, game_id, action)
            .map_err(Error::from)?;
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
        warp::reply::with_header(
            sse::reply(stream),
            "Set-Cookie",
            format!("USER_ID={}", user_id),
        )
    }
    warp::path!("subscribe")
        .and(warp::get())
        .and(warp::any().map(move || state))
        .and(user_id())
        .and(user_name())
        .map(handle)
}

fn run_bot<S: Strategy>(state: &'static Server, name: String, strategy: S) {
    tokio::spawn(async move {
        let hash = Sha256::digest(name.as_bytes());
        let user_id = UserId(RandId::from_hash(&hash));
        let rx = state.subscribe(user_id, name);
        let tx = move |user_id, game_id, action| state.apply_action(user_id, game_id, action);
        let mut bot = Bot::new(user_id, rx, tx, strategy, |action| match action {
            Action::Slough { .. } | Action::Goat { .. } => Duration::from_millis(750),
            Action::PlayCard { .. } | Action::PlayTop => Duration::from_millis(1500),
            Action::PlayRun { .. } | Action::PickUp => Duration::from_secs(3),
            Action::Draw | Action::FinishTrick => Duration::from_millis(200),
            _ => Duration::ZERO,
        });
        if let Err(e) = bot.run().await {
            log::error!("Bot {} failed: {}", user_id, e);
        }
    });
}

#[tokio::main]
async fn main() {
    env_logger::init();
    let state: &Server = &*Box::leak(Box::default());

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
            state.forget_old_state(
                Duration::from_secs(30 * 60),
                Duration::from_secs(18 * 60 * 60),
                Duration::from_secs(5 * 60),
            );
        }
    });

    run_bot(state, "Alice (bot)".to_string(), AdaptSimulate);
    run_bot(state, "Bob (bot)".to_string(), AdaptSimulate);
    run_bot(state, "Carla (bot)".to_string(), AdaptSimulate);
    run_bot(state, "Dimitri (bot)".to_string(), AdaptSimulate);
    run_bot(state, "Eric (bot)".to_string(), AdaptSimulate);
    run_bot(state, "Felicia (bot)".to_string(), AdaptSimulate);
    run_bot(state, "George (bot)".to_string(), AdaptSimulate);
    run_bot(state, "Hannah (bot)".to_string(), AdaptSimulate);

    let app = root()
        .or(assets())
        .or(new_game(state))
        .or(change_name(state))
        .or(apply_action(state))
        .or(subscribe(state))
        .recover(handle_error)
        .with(warp::log("request"));
    warp::serve(app).run(([127, 0, 0, 1], 9402)).await;
}
