use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use log::LevelFilter;
use rand::RngCore;
use uuid::Uuid;

use goat_api::{
    Action, Card, Client, ClientGame, ClientPhase, Event, GoatError, Response, User, UserId,
};
use goat_bot::{Bot, CoverSimple, DuckSimple, PlayTopSimple, Strategy};

use crate::Server;

fn run_bot<S: Strategy + Send + 'static>(state: Arc<Server>, name: String, strategy: S) -> UserId {
    let user_id = UserId(Uuid::new_v4());
    let rx = state.subscribe(user_id, name);
    tokio::spawn(async move {
        let tx = move |user_id, game_id, action| state.apply_action(user_id, game_id, action);
        let mut bot = Bot::new(user_id, rx, tx, strategy, |_| Duration::ZERO);
        if let Err(e) = bot.run().await {
            log::error!("Bot {} failed: {}", user_id, e);
        }
    });
    user_id
}

macro_rules! expect {
    ($rx:ident, $( $response:expr ),* ) => {
        $(
            assert_eq!($rx.recv().await, Some($response));
        )*
    };
}

macro_rules! top {
    ($rx:ident, $game_id:ident, $( $card:tt ),* ) => {
        expect!(
            $rx
            $(, Response::Game {
                $game_id,
                event: Event::PlayTop {
                    card: stringify!($card).parse().unwrap()
                }
            })*
        );
        assert!(matches!($rx.recv().await, Some(Response::Game { event: Event::FinishSloughing { .. }, .. })));
        assert!(matches!($rx.recv().await, Some(Response::Game { event: Event::FinishSloughing { .. }, .. })));
        assert!(matches!($rx.recv().await, Some(Response::Game { event: Event::FinishSloughing { .. }, .. })));
    };
}

macro_rules! run {
    ($rx:ident, $game_id:ident, $lo:tt, $hi:tt) => {
        expect!(
            $rx,
            Response::Game {
                $game_id,
                event: Event::PlayRun {
                    lo: stringify!($lo).parse().unwrap(),
                    hi: stringify!($hi).parse().unwrap(),
                }
            }
        );
    };
}

macro_rules! pick_up {
    ($rx:ident, $game_id:ident) => {
        expect!(
            $rx,
            Response::Game {
                $game_id,
                event: Event::PickUp,
            }
        );
    };
}

#[tokio::test]
async fn test_play_top_deterministic() -> Result<(), GoatError> {
    let server = Arc::new(Server::new());
    let watcher = UserId(Uuid::new_v4());
    let mut rx = server.subscribe(watcher, "watcher".to_string());
    expect!(
        rx,
        Response::User {
            user_id: watcher,
            user: User {
                name: "watcher".to_string(),
                online: true
            },
        }
    );
    let cover = run_bot(server.clone(), "cover".to_string(), PlayTopSimple);
    let duck = run_bot(server.clone(), "duck".to_string(), PlayTopSimple);
    let top = run_bot(server.clone(), "top".to_string(), PlayTopSimple);
    expect!(
        rx,
        Response::User {
            user_id: cover,
            user: User {
                name: "cover".to_string(),
                online: true
            },
        },
        Response::User {
            user_id: duck,
            user: User {
                name: "duck".to_string(),
                online: true
            },
        },
        Response::User {
            user_id: top,
            user: User {
                name: "top".to_string(),
                online: true
            },
        }
    );
    let game_id = server.new_game(1);
    server.apply_action(watcher, game_id, Action::Join { user_id: cover })?;
    server.apply_action(watcher, game_id, Action::Join { user_id: duck })?;
    server.apply_action(watcher, game_id, Action::Join { user_id: top })?;
    expect!(
        rx,
        Response::Replay {
            game_id,
            events: Vec::new(),
        },
        Response::Game {
            game_id,
            event: Event::Join { user_id: cover },
        },
        Response::Game {
            game_id,
            event: Event::Join { user_id: duck },
        },
        Response::Game {
            game_id,
            event: Event::Join { user_id: top },
        }
    );
    server.apply_action(watcher, game_id, Action::Start { num_decks: 1 })?;
    expect!(
        rx,
        Response::Game {
            game_id,
            event: Event::Start { num_decks: 1 },
        }
    );
    top!(rx, game_id, 3C, KC, 6D);
    top!(rx, game_id, 5S, 7H, 4S);
    top!(rx, game_id, 6S, 4C, 3S);
    top!(rx, game_id, 5H, 2C, 8D);
    top!(rx, game_id, QH, TS, 6C);
    top!(rx, game_id, 7S, QS, QD, KD, KH, 8S, TC);
    top!(rx, game_id, 2D, 9D, 8C);
    top!(rx, game_id, 4H, 7D, KS);
    top!(rx, game_id, QC, 4D, 9S);
    top!(rx, game_id, AH, 8H, 2S);
    top!(rx, game_id, 5D, 9C, JD);
    top!(rx, game_id, JC, TH, 3D);
    top!(rx, game_id, AD, 7C, AS, 9H, 5C);
    top!(rx, game_id, JH, 3H, 6H);
    top!(rx, game_id, TD, AC, 2H);
    expect!(
        rx,
        Response::Game {
            game_id,
            event: Event::RevealTrump {
                trump: Card::JackSpades
            },
        }
    );
    run!(rx, game_id, 2H, 2H);
    run!(rx, game_id, 5H, 5H);
    run!(rx, game_id, 6H, 7H);

    run!(rx, game_id, 3D, 3D);
    run!(rx, game_id, 4D, 4D);
    run!(rx, game_id, 6D, 6D);

    run!(rx, game_id, 2C, 3C);
    run!(rx, game_id, 4C, 5C);
    run!(rx, game_id, TC, TC);

    run!(rx, game_id, 4H, 4H);
    run!(rx, game_id, QH, QH);
    run!(rx, game_id, 3S, 3S);

    run!(rx, game_id, 5D, 5D);
    run!(rx, game_id, 7D, 7D);
    run!(rx, game_id, 8D, 9D);

    run!(rx, game_id, 6C, 6C);
    run!(rx, game_id, 7C, 7C);
    run!(rx, game_id, QC, QC);

    run!(rx, game_id, TD, TD);
    run!(rx, game_id, TS, TS);
    run!(rx, game_id, AS, AS);

    run!(rx, game_id, 3H, 3H);
    run!(rx, game_id, 8H, 8H);
    pick_up!(rx, game_id);
    run!(rx, game_id, 9H, JH);
    run!(rx, game_id, KH, AH);

    run!(rx, game_id, QD, KD);
    pick_up!(rx, game_id);

    run!(rx, game_id, JD, JD);
    run!(rx, game_id, 2S, 2S);
    pick_up!(rx, game_id);
    run!(rx, game_id, 4S, 4S);
    run!(rx, game_id, 7S, 7S);

    run!(rx, game_id, AC, AC);
    pick_up!(rx, game_id);

    run!(rx, game_id, 9C, 9C);
    run!(rx, game_id, 8S, 8S);
    pick_up!(rx, game_id);
    pick_up!(rx, game_id);

    run!(rx, game_id, 9S, 9S);
    pick_up!(rx, game_id);

    run!(rx, game_id, JC, JC);
    run!(rx, game_id, QS, QS);
    pick_up!(rx, game_id);
    pick_up!(rx, game_id);

    run!(rx, game_id, KS, KS);
    pick_up!(rx, game_id);

    run!(rx, game_id, AD, AD);
    run!(rx, game_id, 9S, 9S);

    run!(rx, game_id, 8C, 9C);
    run!(rx, game_id, 5S, 5S);

    run!(rx, game_id, 6S, 6S);
    run!(rx, game_id, KS, KS);

    run!(rx, game_id, 2D, 2D);
    run!(rx, game_id, 8S, 8S);

    run!(rx, game_id, QS, QS);
    Ok(())
}

#[tokio::test]
async fn test_bots() -> Result<(), GoatError> {
    let _ = env_logger::builder()
        .filter_level(LevelFilter::Info)
        .is_test(true)
        .try_init();
    let server = Arc::new(Server::new());
    let watcher = UserId(Uuid::new_v4());
    let mut rx = server.subscribe(watcher, "watcher".to_string());
    let mut client: Client<(), (), ()> = Client::new(());
    let cover = run_bot(server.clone(), "cover".to_string(), CoverSimple);
    let duck = run_bot(server.clone(), "duck".to_string(), DuckSimple);
    let top = run_bot(server.clone(), "top".to_string(), PlayTopSimple);
    let mut goat_count = HashMap::new();
    for _ in 0..1000 {
        let game_id = server.new_game(rand::thread_rng().next_u64());
        server.apply_action(watcher, game_id, Action::Join { user_id: cover })?;
        server.apply_action(watcher, game_id, Action::Join { user_id: duck })?;
        server.apply_action(watcher, game_id, Action::Join { user_id: top })?;
        server.apply_action(watcher, game_id, Action::Start { num_decks: 1 })?;
        loop {
            if let Some(ClientGame {
                phase: ClientPhase::Goat(goat),
                ..
            }) = client.games.get(&game_id)
            {
                *goat_count.entry(goat.goat).or_insert(0) += 1;
                break;
            }
            let response = rx.recv().await.unwrap();
            client.apply(response)?;
        }
        server.forget_old_state(Duration::ZERO, Duration::ZERO);
    }
    log::info!("Goats: {:?}", goat_count);
    Ok(())
}
