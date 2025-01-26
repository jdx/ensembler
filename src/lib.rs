#[macro_use]
extern crate log;
mod cmd;
#[cfg_attr(windows, path = "ctrlc_stub.rs")]
mod ctrlc;
mod env;
mod error;
mod exit;
mod multi_progress_report;
mod progress_report;
mod style;

pub use error::{Error, Result};
