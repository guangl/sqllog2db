#[cfg(feature = "tui")]
pub mod ui;

#[cfg(feature = "tui")]
pub mod app;

#[cfg(feature = "tui")]
pub mod progress;

#[cfg(feature = "tui")]
pub use app::TuiApp;

#[cfg(feature = "tui")]
pub use progress::{ProgressEvent, ProgressTracker};

#[cfg(feature = "tui")]
pub use ui::run_tui;
