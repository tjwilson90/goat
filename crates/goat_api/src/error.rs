use thiserror::Error;

use crate::{Card, GameId, PlayerIdx, Rank, UserId};

#[derive(Debug, Error)]
pub enum GoatError {
    #[error("Drawing from the deck is not possible when the deck is empty")]
    CannotDrawFromEmptyDeck,
    #[error("Players cannot hold more than three cards at once")]
    CannotDrawMoreThanThreeCards,
    #[error("Players cannot finish sloughing on a trick until is is complete")]
    CannotFinishSloughingIncompleteTrick,
    #[error("Picking up cards from an emoty trick is not possible")]
    CannotPickUpFromEmptyTrick,
    #[error("Playing from the top of the deck is not possible when the deck is empty")]
    CannotPlayFromEmptyDeck,
    #[error("A range starting with {lo} cannot be played on the current trick")]
    CannotPlayRange { lo: Card },
    #[error("Players cannot slough on a trick after they have finished sloughing")]
    CannotSloughOnEndedTrick,
    #[error("Card {card} cannot be sloughed")]
    IllegalSlough { card: Card },
    #[error("This action cannot be taken at this point in the game")]
    InvalidAction,
    #[error("{game_id} is not a valid game id")]
    InvalidGame { game_id: GameId },
    #[error("Games require at least one deck and can be played with at most three decks")]
    InvalidNumberOfDecks,
    #[error("Games require at least 3 players and can be played with at most 16 players")]
    InvalidNumberOfPlayers,
    #[error("User {user_id} is not a real player in the game")]
    InvalidPlayer { user_id: UserId },
    #[error("The cards {lo} to {hi} do not form a valid range")]
    InvalidRange { lo: Card, hi: Card },
    #[error(
        "Players must play a card with the same rank, {rank}, as the \
        highest card played so far in this round of the current trick"
    )]
    MustMatchRank { rank: Rank },
    #[error("Only the goat may make a goat noise")]
    NoFreeShows,
    #[error("Card {card} is not in the hand")]
    NotYourCard { card: Card },
    #[error("It is not player {player}'s turn")]
    NotYourTurn { player: PlayerIdx },
}
