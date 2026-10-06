//! Optional online play: talking to a ttyp server for dailies and
//! leaderboards, and logging in through GitHub. Nothing here runs unless
//! `server` is set in the config. All network calls happen on background
//! threads (`worker`) and come back to the event loop as `RemoteEvent`s.

pub mod board;
pub mod client;
pub mod device;
pub mod players;
pub mod queue;
pub mod token;
pub mod user;
pub mod worker;

pub use board::{BoardPane, BoardView};
pub use client::{Client, OnlineError};
pub use players::{Follows, PlayersTab, PlayersView};
pub use user::UserView;
pub use worker::{Online, RemoteEvent, Request};
