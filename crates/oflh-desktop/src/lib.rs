//! Desktop application service and IPC contracts, independent of the WebView.
#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used))]

pub mod contract;
pub mod dataset;
#[cfg(any(feature = "desktop", test))]
mod dropped_target;
#[cfg(any(feature = "desktop", test))]
mod instance;
pub mod launch;
mod path_text;
mod recent;
pub mod service;
#[cfg(feature = "desktop")]
pub mod shell;
pub mod startup;
