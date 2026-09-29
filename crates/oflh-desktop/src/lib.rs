//! Desktop application service and IPC contracts, independent of the WebView.
#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used))]

pub mod contract;
pub mod dataset;
mod recent;
pub mod service;
#[cfg(feature = "desktop")]
pub mod shell;
pub mod startup;
