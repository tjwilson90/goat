use thiserror::Error;

use crate::{Card, GameId, PlayerIdx, Rank, UserId};

#[derive(Debug, Error)]
pub enum GoatError {
    #[error("Drawing from the deck is not possible when the deck is empty")]
    CannotDrawFromEmptyDeck,
    #[error("Players cannot hold more than three cards at once")]
    CannotDrawMoreThanThreeCards,
    #[error("Playing from the top of the deck is not possible when the deck is empty")]
    CannotPlayFromEmptyDeck,
    #[error("A range starting with {lo} cannot be played on the current trick")]
    CannotPlayRange { lo: Card },
    #[error("Card {card} cannot be sloughed")]
    IllegalSlough { card: Card },
    #[error("This action cannot be taken at this point in the game")]
    InvalidAction,
    #[error("{game_id} is not a valid game id")]
    InvalidGame { game_id: GameId },
    #[error("At least one deck and at most three decks can be used")]
    InvalidNumberOfDecks,
    #[error("User {user_id} is not a real player in the game")]
    InvalidPlayer { user_id: UserId },
    #[error("The cards {lo} to {hi} do not form a valid range")]
    InvalidRange { lo: Card, hi: Card },
    #[error(
        "Players must play a card with the same rank, {rank}, as the \
        highest card played so far in this round of the current trick"
    )]
    MustMatchRank { rank: Rank },
    #[error("Card {card} is not in the hand")]
    NotYourCard { card: Card },
    #[error("It is not player {player}'s turn")]
    NotYourTurn { player: PlayerIdx },
    #[error("At most 16 players can play in the same game")]
    TooManyPlayers,
}
