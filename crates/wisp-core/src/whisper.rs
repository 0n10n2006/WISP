//! Local speech recognition with Whisper: the model catalogue, the download
//! (checked against the checksum whisper.cpp publishes), WAV decoding and
//! tidying the text Whisper returns. The engine itself lives in the host.

use crate::error::{ErrorCode, Result, WispError};
use crate::settings::{DictationStyle, WhisperModel};
use futures_util::StreamExt;
use regex::Regex;
use sha1::{Digest, Sha1};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use tokio_util::sync::CancellationToken;

/// One downloadable model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModelSpec {
    pub model: WhisperModel,
    pub file: &'static str,
    /// SHA-1 from whisper.cpp's models/README.md.
    pub sha1: &'static str,
    /// Expected size, for progress and as an upper bound while downloading.
    pub bytes: u64,
}

/// whisper.cpp's official model repository.
pub const MODEL_BASE_URL: &str = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/";

pub const MODELS: [ModelSpec; 2] = [
    ModelSpec {
        model: WhisperModel::Standard,
        file: "ggml-small.bin",
        sha1: "55356645c2b361a969dfd0ef2c5a50d530afd8d5",
        bytes: 487_601_967,
    },
    ModelSpec {
        model: WhisperModel::Accurate,
        file: "ggml-large-v3-turbo-q5_0.bin",
        sha1: "e050f7970618a659205450ad97eb95a18d69c9ee",
        bytes: 574_041_195,
    },
];

pub fn spec(model: WhisperModel) -> &'static ModelSpec {
    MODELS
        .iter()
        .find(|m| m.model == model)
        .expect("every model has a spec")
}

/// Where a model lives once downloaded.
pub fn model_path(dir: &Path, model: WhisperModel) -> PathBuf {
    dir.join(spec(model).file)
}

/// Whisper's input rate.
pub const SAMPLE_RATE: u32 = 16_000;
/// Longest recording accepted (dictation is capped at 2 minutes).
pub const MAX_SECONDS: usize = 130;

/// Decode a PCM WAV file to 16 kHz mono samples in -1..1.
pub fn decode_wav(bytes: &[u8]) -> Result<Vec<f32>> {
    let bad = || WispError::invalid("That recording isn't a WAV file Wisp can read.");
    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err(bad());
    }
    let u16_at = |b: &[u8], i: usize| u16::from_le_bytes([b[i], b[i + 1]]);
    let u32_at = |b: &[u8], i: usize| u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]]);
    let mut pos = 12;
    let mut format: Option<(u16, u16, u32, u16)> = None;
    let mut data: Option<&[u8]> = None;
    while pos + 8 <= bytes.len() {
        let id = &bytes[pos..pos + 4];
        let len = u32_at(bytes, pos + 4) as usize;
        let body = pos + 8;
        let end = body.saturating_add(len).min(bytes.len());
        match id {
            b"fmt " if end - body >= 16 => {
                format = Some((
                    u16_at(bytes, body),
                    u16_at(bytes, body + 2),
                    u32_at(bytes, body + 4),
                    u16_at(bytes, body + 14),
                ));
            }
            b"data" => data = Some(&bytes[body..end]),
            _ => {}
        }
        // Chunks are padded to an even length.
        pos = body.saturating_add(len).saturating_add(len & 1);
    }
    let (fmt, channels, rate, bits) = format.ok_or_else(bad)?;
    let data = data.ok_or_else(bad)?;
    if fmt != 1 || bits != 16 || channels == 0 || channels > 2 || !(8_000..=48_000).contains(&rate)
    {
        return Err(bad());
    }
    let frame = 2 * channels as usize;
    let frames = data.len() / frame;
    if frames / rate as usize > MAX_SECONDS {
        return Err(WispError::invalid(
            "That recording is too long. Keep dictation under 2 minutes.",
        ));
    }
    let mono: Vec<f32> = (0..frames)
        .map(|f| {
            let at = f * frame;
            let mut sum = 0.0;
            for c in 0..channels as usize {
                sum += i16::from_le_bytes([data[at + 2 * c], data[at + 2 * c + 1]]) as f32;
            }
            sum / channels as f32 / 32768.0
        })
        .collect();
    Ok(resample(&mono, rate, SAMPLE_RATE))
}

/// Linear resampling (enough for speech; the recorder already sends 16 kHz).
fn resample(input: &[f32], from: u32, to: u32) -> Vec<f32> {
    if from == to || input.is_empty() {
        return input.to_vec();
    }
    let ratio = from as f64 / to as f64;
    let len = (input.len() as f64 / ratio).floor() as usize;
    (0..len)
        .map(|i| {
            let x = i as f64 * ratio;
            let j = x.floor() as usize;
            let t = (x - j as f64) as f32;
            let a = input[j];
            let b = *input.get(j + 1).unwrap_or(&a);
            a + (b - a) * t
        })
        .collect()
}

/// Whisper copies the style and spelling of its prompt, so the user's
/// names and words go there.
pub fn initial_prompt(vocabulary: &[String]) -> String {
    let mut out = String::new();
    for w in vocabulary
        .iter()
        .map(|w| w.trim())
        .filter(|w| !w.is_empty())
    {
        if out.len() + w.len() > 220 {
            break;
        }
        if !out.is_empty() {
            out.push_str(", ");
        }
        out.push_str(w);
    }
    if out.is_empty() {
        out
    } else {
        format!("{out}.")
    }
}

fn re(cell: &'static OnceLock<Regex>, pattern: &str) -> &'static Regex {
    cell.get_or_init(|| Regex::new(pattern).expect("valid regex"))
}

/// Clean up Whisper's text: drop sound tags ("[BLANK_AUDIO]", "(music)")
/// and subtitle sign-offs it invents on silence, and for the Clean style
/// also filler words.
pub fn tidy(text: &str, style: DictationStyle) -> String {
    static TAGS: OnceLock<Regex> = OnceLock::new();
    static SOUNDS: OnceLock<Regex> = OnceLock::new();
    static FILLERS: OnceLock<Regex> = OnceLock::new();
    static SPACES: OnceLock<Regex> = OnceLock::new();
    static PUNCT_SPACE: OnceLock<Regex> = OnceLock::new();
    let mut t = re(&TAGS, r"\[[^\]]*\]|\*[^*]{1,30}\*").replace_all(text, " ");
    t = re(
        &SOUNDS,
        r"(?i)\((?:[^)]*\b(?:music|applause|laugh\w*|silence|inaudible|noise|blank|sigh\w*|cough\w*)\b[^)]*)\)",
    )
    .replace_all(&t, " ")
    .into_owned()
    .into();
    if style == DictationStyle::Clean {
        t = re(
            &FILLERS,
            r"(?i),?\s*\b(?:u+m+|u+h+|e+r+m+|u+h+m+|h+m+)\b[,.]?",
        )
        .replace_all(&t, " ")
        .into_owned()
        .into();
    }
    let t = re(&SPACES, r"\s+").replace_all(&t, " ");
    let t = re(&PUNCT_SPACE, r"\s+([,.!?;:])").replace_all(&t, "$1");
    let mut t = t.trim().trim_start_matches([',', ';', ' ']).to_string();
    let lower = t.to_lowercase();
    const SIGN_OFFS: [&str; 5] = [
        "thanks for watching",
        "thank you for watching",
        "please subscribe",
        "subtitles by",
        "transcribed by",
    ];
    if SIGN_OFFS.iter().any(|s| lower.starts_with(s)) && t.len() < 60 {
        return String::new();
    }
    if style == DictationStyle::Clean {
        if let Some(first) = t.chars().next() {
            if first.is_lowercase() {
                t = first.to_uppercase().collect::<String>() + &t[first.len_utf8()..];
            }
        }
    }
    t
}

/// SHA-1 of a file, as lowercase hex.
pub fn file_sha1(path: &Path) -> std::io::Result<String> {
    let mut f = std::fs::File::open(path)?;
    let mut h = Sha1::new();
    std::io::copy(&mut f, &mut h)?;
    Ok(hex(&h.finalize()))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// HTTP client for model downloads (no overall timeout: the files are big).
pub fn http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(20))
        .read_timeout(std::time::Duration::from_secs(60))
        .user_agent(concat!("Wisp/", env!("CARGO_PKG_VERSION")))
        .build()
        .unwrap_or_default()
}

/// Whisper's language code for a BCP-47 hint ("en-GB" → "en"); "auto"
/// when none is set.
pub fn language_code(hint: &str) -> String {
    let primary = hint.trim().split(['-', '_']).next().unwrap_or("");
    if (2..=3).contains(&primary.len()) && primary.chars().all(|c| c.is_ascii_alphabetic()) {
        primary.to_ascii_lowercase()
    } else {
        "auto".into()
    }
}

/// Download a model into `dir`, reporting (done, total) bytes. The file only
/// gets its final name once its checksum matches.
pub async fn download(
    client: &reqwest::Client,
    base_url: &str,
    model: WhisperModel,
    dir: &Path,
    cancel: &CancellationToken,
    progress: impl FnMut(u64, u64),
) -> Result<PathBuf> {
    fetch(client, base_url, spec(model), dir, cancel, progress).await
}

async fn fetch(
    client: &reqwest::Client,
    base_url: &str,
    spec: &ModelSpec,
    dir: &Path,
    cancel: &CancellationToken,
    mut progress: impl FnMut(u64, u64),
) -> Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let target = dir.join(spec.file);
    let part = dir.join(format!("{}.part", spec.file));
    let url = format!("{base_url}{}", spec.file);
    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|_| offline())?
        .error_for_status()
        .map_err(|e| {
            WispError::new(
                ErrorCode::ServerError,
                format!(
                    "The model download failed ({}). Try again later.",
                    e.status().map(|s| s.as_u16()).unwrap_or(0)
                ),
            )
        })?;
    let total = resp.content_length().unwrap_or(spec.bytes);
    // A little slack over the published size, in case of a re-upload.
    let limit = spec.bytes + spec.bytes / 5;
    if total > limit {
        return Err(WispError::invalid(
            "The model file is larger than expected.",
        ));
    }
    let mut file = std::fs::File::create(&part)?;
    let mut hasher = Sha1::new();
    let mut done = 0u64;
    let mut last = 0u64;
    let mut stream = resp.bytes_stream();
    let result: Result<()> = async {
        loop {
            let chunk = tokio::select! {
                _ = cancel.cancelled() => {
                    return Err(WispError::new(ErrorCode::Cancelled, "Download cancelled"));
                }
                c = stream.next() => c,
            };
            let Some(chunk) = chunk else { break };
            let chunk = chunk.map_err(|_| offline())?;
            done += chunk.len() as u64;
            if done > limit {
                return Err(WispError::invalid(
                    "The model file is larger than expected.",
                ));
            }
            hasher.update(&chunk);
            file.write_all(&chunk)?;
            if done - last >= 1 << 20 {
                last = done;
                progress(done, total);
            }
        }
        file.flush()?;
        Ok(())
    }
    .await;
    drop(file);
    if let Err(e) = result {
        let _ = std::fs::remove_file(&part);
        return Err(e);
    }
    if hex(&hasher.finalize()) != spec.sha1 {
        let _ = std::fs::remove_file(&part);
        return Err(WispError::new(
            ErrorCode::MalformedResponse,
            "The downloaded model didn't match its checksum, so it was deleted. Try again.",
        ));
    }
    std::fs::rename(&part, &target)?;
    progress(done, done);
    Ok(target)
}

fn offline() -> WispError {
    let mut e = WispError::new(
        ErrorCode::Offline,
        "Couldn't download the speech model. Check your internet connection.",
    );
    e.retryable = true;
    e
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wav(rate: u32, channels: u16, samples: &[i16]) -> Vec<u8> {
        let data_len = (samples.len() * 2) as u32;
        let mut v = Vec::new();
        v.extend_from_slice(b"RIFF");
        v.extend_from_slice(&(36 + data_len).to_le_bytes());
        v.extend_from_slice(b"WAVEfmt ");
        v.extend_from_slice(&16u32.to_le_bytes());
        v.extend_from_slice(&1u16.to_le_bytes());
        v.extend_from_slice(&channels.to_le_bytes());
        v.extend_from_slice(&rate.to_le_bytes());
        v.extend_from_slice(&(rate * 2 * channels as u32).to_le_bytes());
        v.extend_from_slice(&(2 * channels).to_le_bytes());
        v.extend_from_slice(&16u16.to_le_bytes());
        // An unrelated chunk with odd length (padded) before the data.
        v.extend_from_slice(b"LIST");
        v.extend_from_slice(&3u32.to_le_bytes());
        v.extend_from_slice(&[1, 2, 3, 0]);
        v.extend_from_slice(b"data");
        v.extend_from_slice(&data_len.to_le_bytes());
        for s in samples {
            v.extend_from_slice(&s.to_le_bytes());
        }
        v
    }

    #[test]
    fn decodes_mono_and_stereo() {
        let m = decode_wav(&wav(16_000, 1, &[0, 16384, -32768])).unwrap();
        assert_eq!(m, vec![0.0, 0.5, -1.0]);
        let s = decode_wav(&wav(16_000, 2, &[16384, 0, -16384, -16384])).unwrap();
        assert_eq!(s, vec![0.25, -0.5]);
        // 32 kHz is halved to 16 kHz.
        let r = decode_wav(&wav(32_000, 1, &[0; 3200])).unwrap();
        assert_eq!(r.len(), 1600);
    }

    #[test]
    fn rejects_other_files() {
        assert!(decode_wav(b"not a wav").is_err());
        let mut w = wav(16_000, 1, &[0; 4]);
        w[20] = 3; // float format
        assert!(decode_wav(&w).is_err());
        let long = wav(8_000, 1, &vec![0; 8_000 * (MAX_SECONDS + 1)]);
        assert!(decode_wav(&long).unwrap_err().message.contains("too long"));
    }

    #[test]
    fn tidies_whisper_output() {
        assert_eq!(tidy(" [BLANK_AUDIO] ", DictationStyle::Clean), "");
        assert_eq!(
            tidy(
                "um, so I think (music) we should, uh, go.",
                DictationStyle::Clean
            ),
            "So I think we should go."
        );
        assert_eq!(
            tidy("um, so we go.", DictationStyle::Verbatim),
            "um, so we go."
        );
        assert_eq!(tidy("Thanks for watching!", DictationStyle::Clean), "");
        assert_eq!(
            tidy(
                "Thanks for watching the kids yesterday, it really helped us out a lot.",
                DictationStyle::Clean
            ),
            "Thanks for watching the kids yesterday, it really helped us out a lot."
        );
        // Words that merely contain filler letters stay.
        assert_eq!(
            tidy("Umbrella and hummus.", DictationStyle::Clean),
            "Umbrella and hummus."
        );
    }

    #[test]
    fn prompt_from_vocabulary() {
        assert_eq!(initial_prompt(&[]), "");
        assert_eq!(
            initial_prompt(&["Wisp".into(), " ".into(), "Ada Lovelace".into()]),
            "Wisp, Ada Lovelace."
        );
        let many: Vec<String> = (0..100).map(|i| format!("word{i}")).collect();
        assert!(initial_prompt(&many).len() <= 230);
    }

    #[test]
    fn language_codes() {
        assert_eq!(language_code("en-GB"), "en");
        assert_eq!(language_code("PT_br"), "pt");
        assert_eq!(language_code(""), "auto");
        assert_eq!(language_code("x"), "auto");
        assert_eq!(language_code("e1-US"), "auto");
    }

    #[test]
    fn models_and_paths() {
        assert_eq!(spec(WhisperModel::Standard).file, "ggml-small.bin");
        assert_eq!(
            model_path(Path::new("/m"), WhisperModel::Accurate),
            Path::new("/m/ggml-large-v3-turbo-q5_0.bin")
        );
        for m in MODELS {
            assert_eq!(m.sha1.len(), 40);
        }
    }

    #[tokio::test]
    async fn download_checks_the_checksum() {
        use wiremock::matchers::path;
        use wiremock::{Mock, MockServer, ResponseTemplate};
        let server = MockServer::start().await;
        Mock::given(path("/ggml-small.bin"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(vec![7u8; 4096]))
            .mount(&server)
            .await;
        let dir = tempfile::tempdir().unwrap();
        let client = reqwest::Client::new();
        let err = download(
            &client,
            &format!("{}/", server.uri()),
            WhisperModel::Standard,
            dir.path(),
            &CancellationToken::new(),
            |_, _| {},
        )
        .await
        .unwrap_err();
        assert!(err.message.contains("checksum"));
        // Nothing is left behind.
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
    }

    #[tokio::test]
    async fn download_keeps_a_verified_file() {
        use wiremock::matchers::path;
        use wiremock::{Mock, MockServer, ResponseTemplate};
        let server = MockServer::start().await;
        Mock::given(path("/m.bin"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(b"abc".to_vec()))
            .mount(&server)
            .await;
        let dir = tempfile::tempdir().unwrap();
        let spec = ModelSpec {
            model: WhisperModel::Standard,
            file: "m.bin",
            sha1: "a9993e364706816aba3e25717850c26c9cd0d89d",
            bytes: 3,
        };
        let mut seen = Vec::new();
        let p = fetch(
            &reqwest::Client::new(),
            &format!("{}/", server.uri()),
            &spec,
            dir.path(),
            &CancellationToken::new(),
            |d, t| seen.push((d, t)),
        )
        .await
        .unwrap();
        assert_eq!(std::fs::read(&p).unwrap(), b"abc");
        assert_eq!(seen.last(), Some(&(3, 3)));
        assert!(!dir.path().join("m.bin.part").exists());
    }

    #[test]
    fn sha1_of_a_file() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("f");
        std::fs::write(&p, b"abc").unwrap();
        assert_eq!(
            file_sha1(&p).unwrap(),
            "a9993e364706816aba3e25717850c26c9cd0d89d"
        );
    }
}
