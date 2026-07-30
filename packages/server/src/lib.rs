mod api_doc;
mod app;
mod chat;
mod health;
mod mdns;
mod sessions;
mod state;
mod terminal;
mod tools;

pub use app::{build, serve};
pub(crate) use state::{AppState, Registry};
