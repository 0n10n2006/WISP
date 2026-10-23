//! Manual update check against the project's GitHub releases (the trusted
//! release channel). Nothing is downloaded or installed automatically.

use crate::error::{ErrorCode, Result, WispError};
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub current: String,
    pub latest: Option<String>,
    pub available: bool,
    /// Release page to open in the browser.
    pub url: Option<String>,
}

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    html_url: String,
    #[serde(default)]
    draft: bool,
}

/// Parse "v1.2.3" / "1.2.3-beta" into a comparable triple.
pub fn parse_version(v: &str) -> Option<(u64, u64, u64)> {
    let core = v.trim().trim_start_matches(['v', 'V']);
    let core = core.split(['-', '+']).next()?;
    let mut parts = core.split('.').map(|p| p.parse::<u64>().ok());
    let major = parts.next()??;
    let minor = parts.next().flatten().unwrap_or(0);
    let patch = parts.next().flatten().unwrap_or(0);
    Some((major, minor, patch))
}

pub fn is_newer(current: &str, latest: &str) -> bool {
    match (parse_version(current), parse_version(latest)) {
        (Some(c), Some(l)) => l > c,
        _ => false,
    }
}

/// Query `{api_base}/repos/{repo}/releases/latest`.
pub async fn check(api_base: &str, repo: &str, current: &str) -> Result<UpdateInfo> {
    let http = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(20))
        .user_agent(concat!("Wisp/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| WispError::internal(e.to_string()))?;
    let url = format!(
        "{}/repos/{repo}/releases/latest",
        api_base.trim_end_matches('/')
    );
    let resp = http
        .get(url)
        .header("accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| crate::gemini::errors::map_transport_error(&e))?;
    let none = UpdateInfo {
        current: current.to_string(),
        latest: None,
        available: false,
        url: None,
    };
    if resp.status().as_u16() == 404 {
        return Ok(none);
    }
    if !resp.status().is_success() {
        return Err(WispError::new(
            ErrorCode::ServerError,
            format!(
                "Couldn't check for updates (HTTP {}). Try again later.",
                resp.status().as_u16()
            ),
        ));
    }
    let r: Release = resp.json().await.map_err(|_| {
        WispError::new(
            ErrorCode::MalformedResponse,
            "Couldn't read the release information.",
        )
    })?;
    if r.draft {
        return Ok(none);
    }
    // Only ever point at this project's own release pages.
    let prefix = format!("https://github.com/{repo}/releases/");
    let url = crate::tools::validate_url(&r.html_url)
        .ok()
        .filter(|u| u.starts_with(&prefix));
    Ok(UpdateInfo {
        current: current.to_string(),
        available: is_newer(current, &r.tag_name) && url.is_some(),
        latest: Some(r.tag_name.trim_start_matches(['v', 'V']).to_string()),
        url,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_versions() {
        assert!(is_newer("0.1.0", "v0.2.0"));
        assert!(is_newer("0.1.9", "0.1.10"));
        assert!(is_newer("1.0.0", "2"));
        assert!(!is_newer("0.2.0", "v0.2.0"));
        assert!(!is_newer("0.3.0", "0.2.9"));
        assert!(!is_newer("0.1.0", "nightly"));
        assert_eq!(parse_version("v1.2.3-beta.1"), Some((1, 2, 3)));
    }
}
