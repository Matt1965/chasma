//! Shared persistence boundaries (player saves and dev adapters).
//!
//! Core types compile without the `dev` feature. Dev scene loading uses the
//! transactional session API via a thin adapter in `dev::scenes`.

mod session;
mod transaction;

pub use session::AuthoritativeSessionSnapshot;
pub use transaction::{TransactionRolledBack, transactional_session_load};

#[cfg(all(test, feature = "dev"))]
mod tests;
