//! Typed commands exposed to the webview. Each is a narrow, validated entry
//! point; nothing here executes model-generated text as a command.

pub mod ask;
pub mod capture;
pub mod dashboard;
pub mod data;
pub mod files;
pub mod local;
pub mod personalize;
pub mod settings;
pub mod system;
pub mod voice;

pub type CmdResult<T> = Result<T, wisp_core::WispError>;
