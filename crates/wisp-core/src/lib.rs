//! Wisp core: platform-independent logic shared by the desktop host.
//!
//! Nothing in this crate touches windows, global shortcuts or other OS
//! integration, so all of it can be unit-tested on any platform.

pub mod capture;
pub mod diagnostics;
pub mod error;
pub mod gemini;
pub mod hold;
pub mod launcher;
pub mod net;
pub mod openrouter;
pub mod redact;
pub mod retry;
pub mod router;
pub mod sanitize;
pub mod screen;
pub mod settings;
pub mod shortcuts;
pub mod storage;
pub mod tools;
pub mod updates;
pub mod winsettings;

pub use error::{ErrorCode, Result, WispError};
