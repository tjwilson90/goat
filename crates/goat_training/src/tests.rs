use super::*;
use rand::{Rng, SeedableRng};

#[test]
fn war_worlds_preserve_public_cards_and_private_views() {
    for (players, decks) in [(3, 1), (6, 1), (4, 2), (6, 3)] {
        let mut env = Environment::new(123, players, decks, true, 0).unwrap();
        for tick in 0..120 {
            let ClientPhase::War(view) = &env.clients[env.player].phase else {
                break;
            };
            let root = PlayerIdx(env.player as u8);
            let mut world = war_advanced::sample_world(view, tick);
            let ServerPhase::War(full) = &world.server.phase else {
                unreachable!()
            };
            let all: Cards = full
                .deck
                .iter()
                .copied()
                .chain(full.hands.iter().flat_map(|h| h.cards()))
                .chain(full.won.iter().flat_map(|c| c.cards()))
                .chain(full.trick.cards())
                .collect();
            assert_eq!(all, Cards::ONE_DECK * decks as usize);
            assert_eq!(war_advanced::deck_count(view), decks as usize);
            for (seat, client) in world.clients.iter().enumerate() {
                let ClientPhase::War(sample) = &client.phase else {
                    unreachable!()
                };
                assert_eq!(sample.won, view.won);
                assert_eq!(
                    sample.trick.cards().collect::<Cards>(),
                    view.trick.cards().collect::<Cards>()
                );
                for (i, hand) in sample.hands.iter().enumerate() {
                    assert_eq!(hand.len(), view.hands[i].len());
                    assert_eq!(matches!(hand, ClientWarHand::Visible(_)), seat == i);
                }
            }
            let ClientPhase::War(sample_root) = &world.clients[env.player].phase else {
                unreachable!()
            };
            assert_eq!(observe_war(root, sample_root), observe_war(root, view));
            let action = war_advanced::policy(root, view);
            world.apply(env.player, action).unwrap();
            // Exercise the real engine and event redaction for each sampled action.
            if let ServerPhase::War(full) = &world.server.phase {
                for (seat, client) in world.clients.iter().enumerate() {
                    let ClientPhase::War(sample) = &client.phase else {
                        unreachable!()
                    };
                    assert_eq!(sample.won, full.won);
                    for (i, hand) in sample.hands.iter().enumerate() {
                        if let ClientWarHand::Visible(visible) = hand {
                            assert!(i == seat || visible.is_empty());
                            assert_eq!(
                                visible.cards().collect::<Cards>(),
                                full.hands[i].cards().collect::<Cards>()
                            );
                        }
                    }
                }
            }
            env.step(action).unwrap();
        }
    }
}

#[test]
fn war_search_is_deterministic_bounded_and_excludes_incomplete_rounds() {
    let mut env = Environment::new(42, 4, 1, true, 0).unwrap();
    for _ in 0..15 {
        env.step(env.observation().2).unwrap();
    }
    let ClientPhase::War(view) = &env.clients[env.player].phase else {
        panic!("expected war")
    };
    let root = PlayerIdx(env.player as u8);
    let config = war_advanced::Config {
        samples: 3,
        max_rollouts: 18,
        ..Default::default()
    };
    let a = war_advanced::search(root, view, 100, &config);
    let b = war_advanced::search(root, view, 100, &config);
    assert_eq!(a.action, b.action);
    assert_eq!(a.estimates, b.estimates);
    assert!(legal_war(root, view).contains(&a.action));
    assert!(a.rollouts <= 18);
    assert!(a.rollouts > 0);
    assert_eq!(a.truncated, 0);
    assert!(a.completed_rounds > 0);
    let truncated = war_advanced::search(
        root,
        view,
        100,
        &war_advanced::Config {
            max_steps: 1,
            ..config
        },
    );
    assert_eq!(truncated.completed_rounds, 0);
    assert!(truncated.estimates.is_empty());
    assert_eq!(truncated.action, war_advanced::policy(root, view));
    assert_eq!(truncated.truncated, truncated.rollouts);
}

#[test]
fn war_configuration_rejects_invalid_budgets() {
    assert!(war_advanced::Config::default().validate().is_ok());
    for text in [
        r#"{"max_steps":0}"#,
        r#"{"max_rollouts":0}"#,
        r#"{"samples":100000}"#,
        r#"{"min_improvement":-1}"#,
    ] {
        assert!(
            serde_json::from_str::<war_advanced::Config>(text)
                .unwrap()
                .validate()
                .is_err()
        );
    }
    assert!(serde_json::from_str::<war_advanced::Config>(r#"{"typo":1}"#).is_err());
}

#[test]
fn war_teachers_are_available_to_imitation_training() {
    for style in [5] {
        let mut env = Environment::new(67, 4, 1, true, style).unwrap();
        for _ in 0..80 {
            if env.goat().is_some() {
                break;
            }
            let (_, legal, teacher, _) = env.observation();
            assert!(legal.contains(&teacher));
            env.step(teacher).unwrap();
        }
    }
    assert!(Environment::new(0, 3, 1, false, 6).is_err());
}

#[test]
fn selected_war_preserves_multideck_fallback() {
    for decks in [2, 3] {
        let mut env = Environment::new(7, 4, decks, true, 0).unwrap();
        for _ in 0..60 {
            let ClientPhase::War(view) = &env.clients[env.player].phase else {
                break;
            };
            let idx = PlayerIdx(env.player as u8);
            let result = war_advanced::search(idx, view, 123, &war_advanced::Config::default());
            assert_eq!(result.rollouts, 0);
            assert_eq!(result.action, war_advanced::policy(idx, view));
            env.step(result.action).unwrap();
        }
    }
}

#[test]
fn action_schema_round_trips_and_includes_every_interval() {
    for index in 0..WAIT {
        assert_eq!(action_index(decode_action(index).unwrap()), index);
    }
    assert!(decode_action(WAIT).is_none());
    assert!(decode_action(ACTIONS).is_none());
}

fn assert_conservation(env: &Environment, decks: usize) {
    match &env.server.phase {
        ServerPhase::War(war) => {
            assert_eq!(
                war.deck.len()
                    + war.hands.iter().map(|h| h.len()).sum::<usize>()
                    + war.won.iter().map(|h| h.len()).sum::<usize>()
                    + war.trick.cards().count(),
                decks * 52
            );
        }
        ServerPhase::Rummy(rummy) => {
            for (idx, client) in env.clients.iter().enumerate() {
                let ClientPhase::Rummy(view) = &client.phase else {
                    panic!("phase mismatch")
                };
                assert_eq!(view.next, rummy.next);
                assert_eq!(view.hands[idx].unknown, 0);
                assert_eq!(view.hands[idx].known, rummy.hands[idx]);
                assert_eq!(
                    1 + view.history.len() + view.hands.iter().map(|h| h.len()).sum::<usize>(),
                    decks * 52
                );
                for (visible, actual) in view.hands.iter().zip(rummy.hands.iter()) {
                    assert_eq!(visible.len(), actual.len());
                    assert!(actual.contains_all(visible.known));
                    assert!(visible.known.contains_all(visible.public));
                }
                for (seat, hand) in view.hands.iter().enumerate() {
                    for other in &env.clients {
                        let ClientPhase::Rummy(other) = &other.phase else {
                            unreachable!()
                        };
                        assert_eq!(hand.public, other.hands[seat].public);
                    }
                    if seat != idx {
                        assert_eq!(hand.known, hand.public);
                    }
                }
            }
        }
        ServerPhase::Goat(goat) => {
            for client in &env.clients {
                let ClientPhase::Goat(view) = &client.phase else {
                    panic!("phase mismatch")
                };
                assert_eq!(view.goat, goat.goat);
            }
        }
        _ => unreachable!(),
    }
}

#[test]
fn deterministic_games_replay_and_conserve_cards() {
    for (players, decks) in [(3, 1), (4, 2), (6, 3), (15, 1)] {
        for seed in 0..6 {
            let mut env = Environment::new(seed, players, decks, true, 0).unwrap();
            let mut replay = Environment::new(seed, players, decks, true, 0).unwrap();
            let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
            while env.goat().is_none() {
                assert_conservation(&env, decks as usize);
                let (obs, legal, teacher, _) = env.observation();
                assert_eq!(obs.len(), OBSERVATIONS);
                assert!(obs.iter().all(|v| v.is_finite()));
                assert!(legal.contains(&teacher));
                assert_eq!(env.observation(), replay.observation());
                // Include exploration, but keep games reasonably short.
                let action = if rng.gen_bool(0.2) {
                    legal[rng.gen_range(0..legal.len())]
                } else {
                    teacher
                };
                env.step(action).unwrap();
                replay.step(action).unwrap();
            }
            assert_conservation(&env, decks as usize);
            assert_eq!(env.server.events, replay.server.events);
            for idx in 0..players {
                let mut reconstructed = ClientGame::<(), Cards>::default();
                for event in &env.server.events {
                    reconstructed
                        .apply(event.redact(Some(PlayerIdx(idx as u8))))
                        .unwrap();
                }
                assert_eq!(
                    format!("{reconstructed:?}"),
                    format!("{:?}", env.clients[idx])
                );
            }
        }
    }
}

#[test]
fn legal_masks_agree_with_authoritative_engine() {
    for (players, decks) in [(3, 1), (4, 2), (6, 3), (15, 1)] {
        let mut env = Environment::new(77, players, decks, true, 0).unwrap();
        let mut visited = 0;
        while env.goat().is_none() && visited < 500 {
            let (_, legal, teacher, phase) = env.observation();
            for &index in &legal {
                if let Some(action) = decode_action(index) {
                    let mut server = env.server.clone();
                    server.apply(server.players[env.player], action).unwrap();
                }
            }
            if phase == 1 {
                for index in 0..RUMMY_ACTIONS {
                    let mut server = env.server.clone();
                    let accepted = server
                        .apply(server.players[env.player], decode_action(index).unwrap())
                        .is_ok();
                    assert_eq!(
                        accepted,
                        legal.contains(&index),
                        "rummy mask disagreed on {index}"
                    );
                }
            }
            env.step(teacher).unwrap();
            visited += 1;
        }
    }
}

#[test]
fn private_draws_do_not_enter_other_players_observations() {
    let base = Environment::new(22, 3, 1, true, 0).unwrap();
    let mut a = base.clients[0].clone();
    let mut b = base.clients[0].clone();
    for (client, card) in [(&mut a, Card::AceClubs), (&mut b, Card::TwoHearts)] {
        client
            .apply(
                Event::Draw {
                    player: PlayerIdx(1),
                    card,
                }
                .redact(Some(PlayerIdx(0))),
            )
            .unwrap();
    }
    let (ClientPhase::War(a), ClientPhase::War(b)) = (&a.phase, &b.phase) else {
        unreachable!()
    };
    assert_eq!(observe_war(PlayerIdx(0), a), observe_war(PlayerIdx(0), b));
    assert_eq!(legal_war(PlayerIdx(0), a), legal_war(PlayerIdx(0), b));
}

#[test]
fn invalid_actions_and_configurations_are_rejected() {
    assert!(Environment::new(0, 2, 1, false, 0).is_err());
    assert!(Environment::new(0, 3, 4, false, 0).is_err());
    let mut env = Environment::new(42, 3, 1, false, 0).unwrap();
    let before = env.server.events.clone();
    assert!(env.step(WAIT).is_err());
    assert_eq!(before, env.server.events);
}

#[test]
fn candidate_observations_round_trip_and_preserve_teacher() {
    for (n, decks) in [(3, 1), (6, 1), (4, 2)] {
        let mut env = Environment::new(17, n, decks, false, 0).unwrap();
        while env.goat().is_none() {
            let (obs, legal, teacher, _) = env.observation();
            let view = rummy_features::from_observation(&obs).unwrap();
            assert_eq!(observe_rummy(&view), obs);
            assert_eq!(legal_rummy(&view), legal);
            assert_eq!(action_index(rummy_simple(&view)), teacher);
            for (action, features) in rummy_features::candidates(&obs) {
                assert!(features.iter().all(|v| v.is_finite()));
                assert_eq!(features[0], (action == teacher) as u8 as f32);
            }
            env.step(teacher).unwrap();
        }
    }
}

#[test]
fn proof_search_finds_immediate_exit_and_does_not_invent_budget_results() {
    let mut env = Environment::new(46, 3, 1, false, 0).unwrap();
    let mut endings = 0;
    while env.goat().is_none() {
        let (_, legal, teacher, _) = env.observation();
        let ClientPhase::Rummy(view) = &env.clients[env.player].phase else {
            unreachable!()
        };
        if view.hands.iter().filter(|h| !h.is_empty()).count() == 2 {
            endings += 1;
            rummy_advanced::clear_proof_cache();
            let safe = rummy_advanced::proved_safe_action(view, teacher, 0);
            if let Some(action) = safe {
                assert!(legal.contains(&action));
                let Action::PlayRun { lo, hi } = decode_action(action).unwrap() else {
                    panic!("unproven pickup");
                };
                assert_eq!(Cards::range(lo, hi), view.hands[env.player].known);
            }
        }
        env.step(teacher).unwrap();
    }
    assert!(endings > 0);
}

#[test]
fn advanced_views_preserve_public_ownership_without_private_leaks() {
    for seed in 0..8 {
        let mut env = Environment::new(seed, 5, 1, false, 0).unwrap();
        for tick in 0..80 {
            if env.goat().is_some() {
                break;
            }
            let ClientPhase::Rummy(view) = &env.clients[env.player].phase else {
                unreachable!()
            };
            let samples = rummy_advanced::sample_views(view, tick);
            for (actor, sample) in samples.iter().enumerate() {
                assert_eq!(sample.hands[actor].unknown, 0);
                for (seat, hand) in sample.hands.iter().enumerate() {
                    assert_eq!(hand.len(), view.hands[seat].len());
                    assert_eq!(hand.public, view.hands[seat].public);
                    if actor != seat {
                        assert_eq!(hand.known, hand.public);
                    }
                }
            }
            assert_eq!(
                samples[env.player].hands[env.player].known,
                view.hands[env.player].known
            );
            let full: Cards = samples
                .iter()
                .enumerate()
                .flat_map(|(i, v)| v.hands[i].known.cards())
                .collect();
            assert_eq!(
                full + view.history + Cards::from(view.trump),
                Cards::ONE_DECK
            );
            env.step(env.observation().2).unwrap();
        }
    }
}

#[test]
fn advanced_search_is_legal_repeatable_and_reports_all_outcomes() {
    for seed in 0..6 {
        let env = Environment::new(seed, 4, 1, false, 0).unwrap();
        let ClientPhase::Rummy(view) = &env.clients[env.player].phase else {
            unreachable!()
        };
        let preferred = env.observation().2;
        for samples in [1, 4] {
            let cfg = rummy_advanced::Config {
                samples,
                endgame_cards: 0,
                ..Default::default()
            };
            let a = rummy_advanced::search(view, 123, preferred, &cfg);
            let b = rummy_advanced::search(view, 123, preferred, &cfg);
            assert_eq!(a.action, b.action);
            assert_eq!(a.estimates, b.estimates);
            let legal = legal_rummy(view);
            assert!(legal.contains(&a.action));
            if legal.len() > 1 {
                assert_eq!(a.estimates.len(), legal.len());
                assert!(a.rollouts == legal.len() * samples || a.rollouts == legal.len());
                assert!(
                    a.estimates
                        .iter()
                        .all(|(_, p, n)| (0. ..=1.).contains(p) && (*n == samples || *n == 1))
                );
            }
        }
        let cfg = rummy_advanced::Config {
            max_rollouts: 1,
            endgame_cards: 0,
            ..Default::default()
        };
        let a = rummy_advanced::search(view, 123, preferred, &cfg);
        assert_eq!(a.action, preferred);
        assert!(a.rollouts <= 1);
    }
}

#[test]
fn default_bot_preserves_evaluated_one_deck_decisions() {
    // Captured before pruning experimental policies and bundling the scorer.
    let expected: &[(usize, u64, usize, u64, usize)] = &[
        (3, 12300000, 176, 13567520617801995045, 0),
        (4, 12300000, 198, 1732634930496585511, 0),
        (5, 12300000, 226, 7917775763477902439, 4),
        (6, 12300000, 315, 4274619766409773508, 4),
    ];
    let bot = Selected::default();
    for &(players, seed, decisions, fingerprint, goat) in expected {
        rummy_advanced::clear_proof_cache();
        let mut env = Environment::new(seed, players, 1, true, 0).unwrap();
        let mut hash = 14695981039346656037u64;
        let mut count = 0;
        while env.goat().is_none() {
            let action = match &env.clients[env.player].phase {
                ClientPhase::War(view) => bot
                    .war(PlayerIdx(env.player as u8), view)
                    .map(action_index)
                    .unwrap_or(WAIT),
                ClientPhase::Rummy(view) => action_index(bot.rummy(view)),
                _ => unreachable!(),
            };
            for value in [env.player, action] {
                hash = (hash ^ value as u64).wrapping_mul(1099511628211);
            }
            env.step(action).unwrap();
            count += 1;
            assert!(
                count <= decisions,
                "default bot deviated from selected trajectory"
            );
        }
        assert_eq!(
            (count, hash, env.goat()),
            (decisions, fingerprint, Some(goat)),
            "{players} players, seed {seed}"
        );
    }
}
