//! Optional network features, each triggered explicitly by the user:
//! weather (Open-Meteo, no key) and an internet speed test.

use crate::error::{ErrorCode, Result, WispError};
use crate::settings::WeatherPlace;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

pub const GEOCODE_BASE: &str = "https://geocoding-api.open-meteo.com";
pub const FORECAST_BASE: &str = "https://api.open-meteo.com";
pub const SPEED_URL: &str = "https://speed.cloudflare.com/__down?bytes=25000000";

fn client(timeout: Duration) -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .timeout(timeout)
        .user_agent(concat!("Wisp/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| WispError::internal(e.to_string()))
}

fn net_err(e: reqwest::Error) -> WispError {
    crate::gemini::errors::map_transport_error(&e)
}

#[derive(Deserialize)]
struct GeoResponse {
    #[serde(default)]
    results: Vec<GeoResult>,
}

#[derive(Deserialize)]
struct GeoResult {
    name: String,
    latitude: f64,
    longitude: f64,
    #[serde(default)]
    country: String,
    #[serde(default)]
    admin1: String,
}

/// Look up places by name for the weather card.
pub async fn search_places(base: &str, query: &str) -> Result<Vec<WeatherPlace>> {
    let q = query.trim();
    if q.chars().count() < 2 || q.chars().count() > 80 {
        return Err(WispError::invalid(
            "Type at least two letters of a town or city",
        ));
    }
    let resp = client(Duration::from_secs(15))?
        .get(format!("{}/v1/search", base.trim_end_matches('/')))
        .query(&[
            ("name", q),
            ("count", "6"),
            ("language", "en"),
            ("format", "json"),
        ])
        .send()
        .await
        .map_err(net_err)?;
    if !resp.status().is_success() {
        return Err(WispError::new(
            ErrorCode::ServerError,
            "The weather service didn't answer. Try again later.",
        ));
    }
    let geo: GeoResponse = resp
        .json()
        .await
        .map_err(|_| WispError::new(ErrorCode::MalformedResponse, "Couldn't read places."))?;
    Ok(geo
        .results
        .into_iter()
        .map(|g| {
            let mut name = g.name;
            for extra in [g.admin1, g.country] {
                if !extra.is_empty() && !name.contains(&extra) {
                    name.push_str(", ");
                    name.push_str(&extra);
                }
            }
            WeatherPlace {
                name,
                latitude: g.latitude,
                longitude: g.longitude,
            }
        })
        .collect())
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DayForecast {
    pub date: String,
    pub min: f64,
    pub max: f64,
    pub code: u16,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Weather {
    pub place: String,
    pub temperature: f64,
    pub wind_kmh: f64,
    pub code: u16,
    pub label: String,
    pub is_day: bool,
    pub days: Vec<DayForecast>,
}

#[derive(Deserialize)]
struct ForecastResponse {
    current: Current,
    daily: Daily,
}

#[derive(Deserialize)]
struct Current {
    temperature_2m: f64,
    weather_code: u16,
    wind_speed_10m: f64,
    #[serde(default)]
    is_day: u8,
}

#[derive(Deserialize)]
struct Daily {
    time: Vec<String>,
    temperature_2m_max: Vec<f64>,
    temperature_2m_min: Vec<f64>,
    weather_code: Vec<u16>,
}

/// WMO weather interpretation codes → short label.
pub fn weather_label(code: u16) -> &'static str {
    match code {
        0 => "Clear",
        1 => "Mainly clear",
        2 => "Partly cloudy",
        3 => "Overcast",
        45 | 48 => "Fog",
        51 | 53 | 55 | 56 | 57 => "Drizzle",
        61 | 63 | 65 | 66 | 67 => "Rain",
        71 | 73 | 75 | 77 => "Snow",
        80..=82 => "Showers",
        85 | 86 => "Snow showers",
        95 | 96 | 99 => "Thunderstorm",
        _ => "Unknown",
    }
}

pub async fn forecast(base: &str, place: &WeatherPlace) -> Result<Weather> {
    let resp = client(Duration::from_secs(15))?
        .get(format!("{}/v1/forecast", base.trim_end_matches('/')))
        .query(&[
            ("latitude", place.latitude.to_string()),
            ("longitude", place.longitude.to_string()),
            (
                "current",
                "temperature_2m,weather_code,wind_speed_10m,is_day".into(),
            ),
            (
                "daily",
                "temperature_2m_max,temperature_2m_min,weather_code".into(),
            ),
            ("timezone", "auto".into()),
            ("forecast_days", "5".into()),
        ])
        .send()
        .await
        .map_err(net_err)?;
    if !resp.status().is_success() {
        return Err(WispError::new(
            ErrorCode::ServerError,
            "The weather service didn't answer. Try again later.",
        ));
    }
    let f: ForecastResponse = resp
        .json()
        .await
        .map_err(|_| WispError::new(ErrorCode::MalformedResponse, "Couldn't read the forecast."))?;
    let days = f
        .daily
        .time
        .iter()
        .enumerate()
        .filter_map(|(i, date)| {
            let code = *f.daily.weather_code.get(i)?;
            Some(DayForecast {
                date: date.clone(),
                min: *f.daily.temperature_2m_min.get(i)?,
                max: *f.daily.temperature_2m_max.get(i)?,
                code,
                label: weather_label(code).into(),
            })
        })
        .collect();
    Ok(Weather {
        place: place.name.clone(),
        temperature: f.current.temperature_2m,
        wind_kmh: f.current.wind_speed_10m,
        code: f.current.weather_code,
        label: weather_label(f.current.weather_code).into(),
        is_day: f.current.is_day == 1,
        days,
    })
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpeedResult {
    pub mbps: f64,
    pub bytes: u64,
    pub millis: u64,
    pub latency_ms: u64,
}

pub fn mbps(bytes: u64, elapsed: Duration) -> f64 {
    let secs = elapsed.as_secs_f64().max(0.001);
    (bytes as f64 * 8.0 / 1_000_000.0) / secs
}

/// Download test: streams up to the full body (or 12 s) and reports Mb/s.
pub async fn speed_test(url: &str) -> Result<SpeedResult> {
    let http = client(Duration::from_secs(30))?;
    let started = Instant::now();
    let resp = http.get(url).send().await.map_err(net_err)?;
    let latency_ms = started.elapsed().as_millis() as u64;
    if !resp.status().is_success() {
        return Err(WispError::new(
            ErrorCode::ServerError,
            "The speed test server didn't answer.",
        ));
    }
    let body_start = Instant::now();
    let mut bytes = 0u64;
    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        bytes += chunk.map_err(net_err)?.len() as u64;
        if body_start.elapsed() > Duration::from_secs(12) {
            break;
        }
    }
    let elapsed = body_start.elapsed();
    Ok(SpeedResult {
        mbps: (mbps(bytes, elapsed) * 10.0).round() / 10.0,
        bytes,
        millis: elapsed.as_millis() as u64,
        latency_ms,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_and_maths() {
        assert_eq!(weather_label(0), "Clear");
        assert_eq!(weather_label(81), "Showers");
        assert_eq!(weather_label(1234), "Unknown");
        assert!((mbps(12_500_000, Duration::from_secs(1)) - 100.0).abs() < 1e-9);
    }
}
