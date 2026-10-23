//! API key storage in the OS credential vault (Windows Credential Manager):
//! the Gemini key, and optionally an OpenRouter key.
//!
//! The key is cached in memory after first use and is never returned to the
//! webview, written to disk in plaintext, or logged.

use std::sync::Mutex;
use wisp_core::{ErrorCode, Result, WispError};

const SERVICE: &str = "app.wisp.desktop";
const GEMINI_ACCOUNT: &str = "gemini-api-key";
const OPENROUTER_ACCOUNT: &str = "openrouter-api-key";

pub struct SecretStore {
    account: &'static str,
    cache: Mutex<Option<Option<String>>>,
}

impl Default for SecretStore {
    fn default() -> Self {
        Self::gemini()
    }
}

fn vault_error(e: keyring::Error) -> WispError {
    WispError::new(
        ErrorCode::Storage,
        format!("Couldn't access Windows Credential Manager ({e}). Your key was not saved."),
    )
}

impl SecretStore {
    pub fn gemini() -> Self {
        Self {
            account: GEMINI_ACCOUNT,
            cache: Mutex::new(None),
        }
    }

    pub fn openrouter() -> Self {
        Self {
            account: OPENROUTER_ACCOUNT,
            cache: Mutex::new(None),
        }
    }

    fn entry(&self) -> Result<keyring::Entry> {
        keyring::Entry::new(SERVICE, self.account).map_err(vault_error)
    }

    fn validate(&self, key: &str) -> Result<String> {
        if self.account == OPENROUTER_ACCOUNT {
            if !wisp_core::openrouter::valid_key(key) {
                return Err(WispError::invalid(
                    "That doesn't look like an OpenRouter key (it starts with sk-or-). Copy it again from openrouter.ai/keys.",
                ));
            }
            return Ok(key.trim().to_string());
        }
        validate_key(key)
    }

    pub fn get(&self) -> Result<Option<String>> {
        let mut cache = self.cache.lock().expect("secret cache");
        if let Some(v) = cache.as_ref() {
            return Ok(v.clone());
        }
        let value = match self.entry()?.get_password() {
            Ok(v) => Some(v),
            Err(keyring::Error::NoEntry) => None,
            Err(e) => return Err(vault_error(e)),
        };
        *cache = Some(value.clone());
        Ok(value)
    }

    pub fn set(&self, key: &str) -> Result<()> {
        let key = self.validate(key)?;
        self.entry()?.set_password(&key).map_err(vault_error)?;
        *self.cache.lock().expect("secret cache") = Some(Some(key));
        Ok(())
    }

    pub fn delete(&self) -> Result<()> {
        match self.entry()?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => {}
            Err(e) => return Err(vault_error(e)),
        }
        *self.cache.lock().expect("secret cache") = Some(None);
        Ok(())
    }

    /// The key, or a `MissingKey` error.
    pub fn require(&self) -> Result<String> {
        self.get()?.ok_or_else(|| {
            WispError::new(
                ErrorCode::MissingKey,
                "Add your Gemini API key in Settings → Model to ask Wisp.",
            )
        })
    }
}

pub fn validate_key(key: &str) -> Result<String> {
    let k = key.trim();
    if k.len() < 20 || k.len() > 200 || k.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err(WispError::invalid(
            "That doesn't look like a Gemini API key. Copy it again from Google AI Studio.",
        ));
    }
    Ok(k.to_string())
}
