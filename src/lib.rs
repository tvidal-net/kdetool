use std::time::Duration;

pub mod app;
pub mod cmd;
pub mod geometry;
pub mod kwin;
pub mod model;
pub mod proc;
pub mod service;

pub use app::run;
pub use geometry::{Geometry, Length, Maximize};
pub use model::{Action, Pattern, Search};

/// Maximum time to wait for the KWin script to reply before giving up.
pub const TIMEOUT: Duration = Duration::from_secs(3);
