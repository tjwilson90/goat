use criterion::{criterion_group, criterion_main, BatchSize, Criterion};
use goat_api::{
    Card, Cards, ClientGame, ClientRummyHand, Event, PlayerIdx, RummyPhase, RummyTrick, Suit,
    UserId,
};
use goat_bot::simulate_once;
use rand::SeedableRng;
use std::str::FromStr;

fn cards_benchmark(c: &mut Criterion) {
    c.bench_function("Cards::range", |b| {
        b.iter_batched(
            || (Card::TwoHearts, Card::AceHearts),
            |(lo, hi)| Cards::range(lo, hi),
            BatchSize::SmallInput,
        )
    });
    c.bench_function("Cards::len", |b| {
        b.iter_batched(
            || Cards::from_str("J872H 76C").unwrap(),
            |cards| cards.len(),
            BatchSize::SmallInput,
        )
    });
    c.bench_function("Cards::max", |b| {
        b.iter_batched(
            || Cards::from_str("J872H 76C").unwrap(),
            |cards| cards.max(),
            BatchSize::SmallInput,
        )
    });
    c.bench_function("Cards::min", |b| {
        b.iter_batched(
            || Cards::from_str("J872H 76C").unwrap(),
            |cards| cards.min(),
            BatchSize::SmallInput,
        )
    });
    c.bench_function("Cards::in_suit", |b| {
        b.iter_batched(
            || (Cards::from_str("J872H 76C").unwrap(), Suit::Hearts),
            |(cards, suit)| cards.in_suit(suit),
            BatchSize::SmallInput,
        )
    });
    c.bench_function("Cards::above", |b| {
        b.iter_batched(
            || (Cards::from_str("J872H 76C").unwrap(), Card::SevenHearts),
            |(cards, card)| cards.above(card),
            BatchSize::SmallInput,
        )
    });
    c.bench_function("Cards::below", |b| {
        b.iter_batched(
            || (Cards::from_str("J872H 76C").unwrap(), Card::TenHearts),
            |(cards, card)| cards.below(card),
            BatchSize::SmallInput,
        )
    });
    c.bench_function("Cards::contains", |b| {
        b.iter_batched(
            || (Cards::from_str("J872H 76C").unwrap(), Card::TwoDiamonds),
            |(cards, card)| cards.contains(card),
            BatchSize::SmallInput,
        )
    });
    c.bench_function("Cards::contains_all", |b| {
        b.iter_batched(
            || {
                (
                    Cards::from_str("J872H 76C").unwrap(),
                    Cards::from_str("J82H 6C").unwrap(),
                )
            },
            |(c1, c2)| c1.contains_all(c2),
            BatchSize::SmallInput,
        )
    });
    c.bench_function("Cards::remove_all", |b| {
        b.iter_batched(
            || {
                (
                    Cards::from_str("J872H 76C").unwrap(),
                    Cards::from_str("J82H 3D 6C").unwrap(),
                )
            },
            |(mut c1, c2)| c1.remove_all(c2),
            BatchSize::SmallInput,
        )
    });
    c.bench_function("Cards::top_of_run", |b| {
        b.iter_batched(
            || (Cards::from_str("J8762H 76C").unwrap(), Card::SixHearts),
            |(cards, card)| cards.top_of_run(card),
            BatchSize::SmallInput,
        )
    });
    c.bench_function("Cards::cards count", |b| {
        b.iter_batched(
            || Cards::from_str("J8762H 76C").unwrap(),
            |cards| cards.cards().count(),
            BatchSize::SmallInput,
        )
    });
    c.bench_function("Cards::runs count", |b| {
        b.iter_batched(
            || Cards::from_str("J8762H 76C").unwrap(),
            |cards| cards.runs().count(),
            BatchSize::SmallInput,
        )
    });
}

fn client_apply_benchmark(c: &mut Criterion) {
    fn join(game: &mut ClientGame<(), ()>) {
        for _ in 0..4 {
            game.apply(Event::Join {
                user_id: UserId(rand::random()),
            })
            .unwrap();
        }
    }
    fn start(game: &mut ClientGame<(), ()>) {
        game.apply(Event::Start { num_decks: 1 }).unwrap()
    }
    c.bench_function("Client::apply join", |b| {
        b.iter_batched(
            || (ClientGame::<(), ()>::default(), UserId(rand::random())),
            |(mut game, user_id)| game.apply(Event::Join { user_id }),
            BatchSize::SmallInput,
        )
    });
    c.bench_function("Client::apply leave", |b| {
        b.iter_batched(
            || {
                let mut game = ClientGame::default();
                join(&mut game);
                game
            },
            |mut game| {
                game.apply(Event::Leave {
                    player: PlayerIdx(0),
                })
                .unwrap()
            },
            BatchSize::SmallInput,
        )
    });
    c.bench_function("Client::apply start", |b| {
        b.iter_batched(
            || {
                let mut game = ClientGame::default();
                join(&mut game);
                game
            },
            |mut game| start(&mut game),
            BatchSize::SmallInput,
        )
    });
    c.bench_function("Client::apply draw", |b| {
        b.iter_batched(
            || {
                let mut game = ClientGame::default();
                join(&mut game);
                start(&mut game);
                game
            },
            |mut game| {
                game.apply(Event::Draw {
                    player: PlayerIdx(0),
                    card: Card::FourClubs,
                })
                .unwrap()
            },
            BatchSize::SmallInput,
        )
    });
}

fn rummy_trick_benchmark(c: &mut Criterion) {
    fn disconnected() -> RummyTrick {
        let mut trick = RummyTrick::new(4);
        trick.play(Card::FourClubs, Card::SixClubs);
        trick.play(Card::AceClubs, Card::AceClubs);
        trick.play(Card::ThreeDiamonds, Card::ThreeDiamonds);
        trick
    }
    fn connected() -> RummyTrick {
        let mut trick = RummyTrick::new(4);
        trick.play(Card::FourClubs, Card::SixClubs);
        trick.play(Card::SevenClubs, Card::SevenClubs);
        trick.play(Card::EightClubs, Card::EightClubs);
        trick
    }
    c.bench_function("RummyTrick::len", |b| {
        b.iter_batched(disconnected, |trick| trick.len(), BatchSize::SmallInput)
    });
    c.bench_function("RummyTrick::top_card", |b| {
        b.iter_batched(
            disconnected,
            |trick| trick.top_card(),
            BatchSize::SmallInput,
        )
    });
    c.bench_function("RummyTrick::can_play", |b| {
        b.iter_batched(
            disconnected,
            |trick| trick.can_play(Card::EightDiamonds, Suit::Diamonds),
            BatchSize::SmallInput,
        )
    });
    c.bench_function("RummyTrick::is_empty", |b| {
        b.iter_batched(
            disconnected,
            |trick| trick.is_empty(),
            BatchSize::SmallInput,
        )
    });
    c.bench_function("RummyTrick::num_players", |b| {
        b.iter_batched(
            disconnected,
            |trick| trick.num_players(),
            BatchSize::SmallInput,
        )
    });
    c.bench_function("RummyTrick::pick_up disconnected", |b| {
        b.iter_batched(
            disconnected,
            |mut trick| trick.pick_up(),
            BatchSize::SmallInput,
        )
    });
    c.bench_function("RummyTrick::pick_up connected", |b| {
        b.iter_batched(
            connected,
            |mut trick| trick.pick_up(),
            BatchSize::SmallInput,
        )
    });
    c.bench_function("RummyTrick::play", |b| {
        b.iter_batched(
            disconnected,
            |mut trick| trick.play(Card::AceDiamonds, Card::AceDiamonds),
            BatchSize::SmallInput,
        )
    });
}

fn rummy_phase_benchmark(c: &mut Criterion) {
    fn setup() -> RummyPhase<ClientRummyHand, Cards> {
        let mut phase = RummyPhase::new(
            Box::new([
                ClientRummyHand {
                    known: "AKKQQJJTT998877S KQJH KQJD AKC".parse().unwrap(),
                    unknown: 2,
                },
                ClientRummyHand {
                    known: "AS KQJ9854D AK87652C".parse().unwrap(),
                    unknown: 4,
                },
                ClientRummyHand {
                    known: "6S ATH TTD JJC".parse().unwrap(),
                    unknown: 0,
                },
                ClientRummyHand {
                    known: "4332S AA42D".parse().unwrap(),
                    unknown: 0,
                },
            ]),
            PlayerIdx(2),
            Card::FourSpades,
        );
        phase
            .play_run(PlayerIdx(2), Card::TenDiamonds, Card::TenDiamonds)
            .unwrap();
        phase
            .play_run(PlayerIdx(3), Card::AceDiamonds, Card::AceDiamonds)
            .unwrap();
        phase
    }
    c.bench_function("RummyPhase::play", |b| {
        b.iter_batched(
            setup,
            |mut phase| {
                phase
                    .play_run(PlayerIdx(0), Card::SevenSpades, Card::SevenSpades)
                    .unwrap()
            },
            BatchSize::SmallInput,
        )
    });
    c.bench_function("RummyPhase::kill", |b| {
        b.iter_batched(
            || {
                let mut phase = setup();
                phase
                    .play_run(PlayerIdx(0), Card::SevenSpades, Card::SevenSpades)
                    .unwrap();
                phase
            },
            |mut phase| {
                phase
                    .play_run(PlayerIdx(1), Card::AceSpades, Card::AceSpades)
                    .unwrap()
            },
            BatchSize::SmallInput,
        )
    });
    c.bench_function("RummyPhase::pick_up", |b| {
        b.iter_batched(
            setup,
            |mut phase| phase.pick_up(PlayerIdx(0)).unwrap(),
            BatchSize::SmallInput,
        )
    });
    c.bench_function("RummyPhase::is_finished", |b| {
        b.iter_batched(setup, |phase| phase.is_finished(), BatchSize::SmallInput)
    });
    c.bench_function("simulate_once", |b| {
        b.iter_batched(
            || {
                (
                    rand::rngs::StdRng::seed_from_u64(123),
                    setup(),
                    "765432C".parse().unwrap(),
                )
            },
            |(mut rng, phase, cards)| simulate_once(&mut rng, phase, cards),
            BatchSize::SmallInput,
        )
    });
}

criterion_group!(
    benches,
    cards_benchmark,
    client_apply_benchmark,
    rummy_trick_benchmark,
    rummy_phase_benchmark,
);
criterion_main!(benches);
