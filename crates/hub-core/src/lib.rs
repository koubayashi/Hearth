//! Core data model and local library database for Hearth.

pub mod db;
pub mod model;
pub mod playtime;

pub use db::{Db, DbError};
pub use model::*;
