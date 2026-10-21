//! Typed commands exposed to the webview. Each is a narrow, validated entry
//! point; nothing here executes model-generated text as a command.

pub mod ask;
pub mod capture;
pub mod data;
pub mod settings;
pub mod system;

pub type CmdResult<T> = Result<T, wisp_core::WispError>;
