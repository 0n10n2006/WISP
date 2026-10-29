//! Voice: Windows' built-in speech recognizer (the same service as Windows
//! dictation) turns speech into text with no window of its own. Words arrive
//! live while the user talks; each finished phrase is handed over as soon as
//! it's recognised, and the session ends by itself after a short pause.
//! Wisp never records or stores audio.

use std::time::{Duration, Instant};
use wisp_core::Result;

/// Silence after speech that ends the session.
pub const END_SILENCE: Duration = Duration::from_millis(1200);
/// How long to wait for the first word before giving up.
pub const START_SILENCE: Duration = Duration::from_secs(6);
/// Longest a session may run, whatever Windows thinks it hears.
pub const MAX_SESSION: Duration = Duration::from_secs(60);

/// Callbacks for one listening session (called on Windows' threads).
#[cfg_attr(not(windows), allow(dead_code))]
pub struct Events {
    /// Everything heard so far, including the phrase still being spoken.
    pub live: Box<dyn Fn(String) + Send + Sync>,
    /// A finished phrase, tidied, as it should be typed after the previous one.
    pub phrase: Box<dyn Fn(String) + Send + Sync>,
    /// The session ended: everything that was heard, and why it ended if
    /// something went wrong (no network, no microphone…).
    pub done: Box<dyn Fn(String, Option<String>) + Send + Sync>,
}

/// The phrases heard in one session.
#[derive(Default)]
#[cfg_attr(not(windows), allow(dead_code))]
pub struct Transcript {
    text: String,
    pub partial: String,
}

#[cfg_attr(not(windows), allow(dead_code))]
impl Transcript {
    /// Add a finished phrase; returns it as it should be typed (with a
    /// leading space after earlier text, capitalised at a sentence start).
    pub fn push(&mut self, phrase: &str) -> Option<String> {
        self.partial.clear();
        let p = phrase.trim();
        if p.is_empty() {
            return None;
        }
        let sentence_start =
            self.text.is_empty() || self.text.trim_end().ends_with(['.', '!', '?', '…', '\n']);
        let mut piece = String::new();
        if !self.text.is_empty() {
            piece.push(' ');
        }
        let mut chars = p.chars();
        if let Some(first) = chars.next() {
            if sentence_start {
                piece.extend(first.to_uppercase());
            } else {
                piece.push(first);
            }
            piece.push_str(chars.as_str());
        }
        self.text.push_str(&piece);
        Some(piece)
    }

    /// What to show while the user is still speaking.
    pub fn live(&self) -> String {
        let p = self.partial.trim();
        match (self.text.is_empty(), p.is_empty()) {
            (_, true) => self.text.clone(),
            (true, false) => p.to_string(),
            (false, false) => format!("{} {p}", self.text),
        }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    /// The words still being spoken when listening stops count as said.
    pub fn flush(&mut self) -> Option<String> {
        let p = std::mem::take(&mut self.partial);
        self.push(&p)
    }

    /// A new guess at the words being spoken; true if it changed (only a
    /// change means the user is still talking, not background noise).
    pub fn guess(&mut self, text: &str) -> bool {
        let t = text.trim();
        if t.is_empty() || t == self.partial {
            return false;
        }
        self.partial = t.to_string();
        true
    }
}

/// When to end a session: after a pause once something was said, or when
/// nothing is said at all.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn should_end(started: Instant, last_word: Option<Instant>, now: Instant) -> bool {
    if now.duration_since(started) >= MAX_SESSION {
        return true;
    }
    match last_word {
        Some(t) => now.duration_since(t) >= END_SILENCE,
        None => now.duration_since(started) >= START_SILENCE,
    }
}

#[cfg(windows)]
mod imp {
    use super::*;
    use std::sync::{Arc, Mutex};
    use windows::core::{Interface, HSTRING};
    use windows::Foundation::TypedEventHandler;
    use windows::Media::SpeechRecognition::{
        ISpeechRecognitionConstraint, SpeechContinuousRecognitionCompletedEventArgs,
        SpeechContinuousRecognitionResultGeneratedEventArgs, SpeechContinuousRecognitionSession,
        SpeechRecognitionConfidence, SpeechRecognitionHypothesisGeneratedEventArgs,
        SpeechRecognitionResultStatus, SpeechRecognitionScenario, SpeechRecognitionTopicConstraint,
        SpeechRecognizer, SpeechRecognizerState,
    };
    use wisp_core::{ErrorCode, WispError};

    /// HRESULT Windows returns when online speech recognition is turned off.
    const PRIVACY_DECLINED: i32 = 0x8004_5509_u32 as i32;
    /// Access denied: desktop apps may not use the microphone.
    const ACCESS_DENIED: i32 = 0x8007_0005_u32 as i32;

    struct Shared {
        transcript: Transcript,
        last_word: Option<Instant>,
        events: Option<Events>,
    }

    struct Live {
        recognizer: SpeechRecognizer,
        session: SpeechContinuousRecognitionSession,
        shared: Arc<Mutex<Shared>>,
        tokens: (i64, i64, i64),
    }

    /// A compiled recognizer, kept between uses (it hears nothing until started).
    static READY: Mutex<Option<SpeechRecognizer>> = Mutex::new(None);
    static LIVE: Mutex<Option<Live>> = Mutex::new(None);

    fn init_com() {
        unsafe {
            use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        }
    }

    fn explain(e: &windows::core::Error) -> WispError {
        match e.code().0 {
            PRIVACY_DECLINED => WispError::new(
                ErrorCode::PermissionDenied,
                "Voice needs \"Online speech recognition\" turned on in Windows Settings → Privacy & security → Speech.",
            ),
            ACCESS_DENIED => WispError::new(
                ErrorCode::PermissionDenied,
                "Windows isn't letting apps use the microphone. Turn on \"Let desktop apps access your microphone\" in Settings → Privacy & security → Microphone.",
            ),
            _ => WispError::new(
                ErrorCode::Unsupported,
                format!(
                    "Windows speech recognition couldn't start ({}). Check that a microphone is connected, then try again.",
                    e.code()
                ),
            ),
        }
    }

    /// Why a session ended early, in words (None: it ended normally).
    fn ended_because(status: SpeechRecognitionResultStatus) -> Option<String> {
        let why = match status {
            SpeechRecognitionResultStatus::Success
            | SpeechRecognitionResultStatus::UserCanceled
            | SpeechRecognitionResultStatus::TimeoutExceeded
            | SpeechRecognitionResultStatus::PauseLimitExceeded => return None,
            SpeechRecognitionResultStatus::NetworkFailure => {
                "Couldn't reach Microsoft's speech service. Check your internet connection."
            }
            SpeechRecognitionResultStatus::MicrophoneUnavailable => {
                "No microphone is available. Plug one in or pick one in Windows Settings → Sound."
            }
            SpeechRecognitionResultStatus::AudioQualityFailure => {
                "Windows couldn't hear clearly. Try speaking a little closer to the microphone."
            }
            _ => "Windows stopped listening unexpectedly. Try again.",
        };
        Some(why.to_string())
    }

    fn compile_new() -> Result<SpeechRecognizer> {
        let rec = SpeechRecognizer::new().map_err(|e| explain(&e))?;
        let topic = SpeechRecognitionTopicConstraint::Create(
            SpeechRecognitionScenario::Dictation,
            &HSTRING::from("dictation"),
        )
        .map_err(|e| explain(&e))?;
        rec.Constraints()
            .and_then(|c| c.Append(&topic.cast::<ISpeechRecognitionConstraint>()?))
            .map_err(|e| explain(&e))?;
        let result = rec
            .CompileConstraintsAsync()
            .and_then(|op| op.join())
            .map_err(|e| explain(&e))?;
        let status = result.Status().map_err(|e| explain(&e))?;
        if status != SpeechRecognitionResultStatus::Success {
            return Err(WispError::new(
                ErrorCode::Unsupported,
                if status == SpeechRecognitionResultStatus::TopicLanguageNotSupported {
                    "Windows has no speech recognition for your speech language. Add it in Settings → Time & language → Speech.".to_string()
                } else if status == SpeechRecognitionResultStatus::NetworkFailure {
                    "Couldn't reach Microsoft's speech service. Check your internet connection."
                        .to_string()
                } else {
                    format!("Windows speech recognition isn't ready ({}).", status.0)
                },
            ));
        }
        // Wisp ends the session itself after a short pause (see
        // `should_end`); Windows' own limit is only a backstop.
        if let Ok(s) = rec.ContinuousRecognitionSession() {
            let _ = s.SetAutoStopSilenceTimeout(Duration::from_secs(30).into());
        }
        Ok(rec)
    }

    /// The kept recognizer, once it's idle; a fresh one if it isn't (a
    /// previous session still winding down, or a changed microphone).
    fn recognizer(fresh: bool) -> Result<SpeechRecognizer> {
        let kept = if fresh {
            None
        } else {
            READY.lock().expect("speech").clone()
        };
        if let Some(rec) = kept {
            for _ in 0..15 {
                if rec.State().is_ok_and(|s| s == SpeechRecognizerState::Idle) {
                    return Ok(rec);
                }
                std::thread::sleep(Duration::from_millis(100));
            }
        }
        let rec = compile_new()?;
        if let Some(old) = READY.lock().expect("speech").replace(rec.clone()) {
            let _ = old.Close();
        }
        Ok(rec)
    }

    /// Compile the recognizer ahead of time so the first start is instant.
    pub fn prepare() -> Result<()> {
        init_com();
        recognizer(false).map(|_| ())
    }

    pub fn start(events: Events) -> Result<()> {
        init_com();
        cancel();
        let shared = Arc::new(Mutex::new(Shared {
            transcript: Transcript::default(),
            last_word: None,
            events: Some(events),
        }));
        // A recognizer that won't start is replaced and tried once more.
        let live = match begin(recognizer(false)?, &shared) {
            Ok(l) => l,
            Err(_) => begin(recognizer(true)?, &shared).map_err(|e| explain(&e))?,
        };
        *LIVE.lock().expect("speech") = Some(live);

        // End the session after a pause (or if nothing is said).
        let started = Instant::now();
        std::thread::spawn(move || loop {
            std::thread::sleep(Duration::from_millis(100));
            let (done, end) = {
                let s = shared.lock().expect("speech");
                (
                    s.events.is_none(),
                    should_end(started, s.last_word, Instant::now()),
                )
            };
            if done {
                break;
            }
            if end {
                stop();
                break;
            }
        });
        Ok(())
    }

    fn begin(rec: SpeechRecognizer, shared: &Arc<Mutex<Shared>>) -> windows::core::Result<Live> {
        let session = rec.ContinuousRecognitionSession()?;
        let s = shared.clone();
        let hyp = rec.HypothesisGenerated(&TypedEventHandler::<
            SpeechRecognizer,
            SpeechRecognitionHypothesisGeneratedEventArgs,
        >::new(move |_, args| {
            if let Some(text) = args
                .as_ref()
                .and_then(|a| a.Hypothesis().ok())
                .and_then(|h| h.Text().ok())
            {
                let mut s = s.lock().expect("speech");
                // Only new words mean the user is still talking.
                if s.transcript.guess(&text.to_string()) {
                    s.last_word = Some(Instant::now());
                    let live = s.transcript.live();
                    if let Some(e) = &s.events {
                        (e.live)(live);
                    }
                }
            }
            Ok(())
        }))?;

        let s = shared.clone();
        let res = session.ResultGenerated(&TypedEventHandler::<
            SpeechContinuousRecognitionSession,
            SpeechContinuousRecognitionResultGeneratedEventArgs,
        >::new(move |_, args| {
            if let Some(r) = args.as_ref().and_then(|a| a.Result().ok()) {
                let rejected = r
                    .Confidence()
                    .map(|c| c == SpeechRecognitionConfidence::Rejected)
                    .unwrap_or(true);
                let text = r.Text().map(|t| t.to_string()).unwrap_or_default();
                let mut s = s.lock().expect("speech");
                // Noise Windows rejects doesn't keep the session going.
                let piece = if rejected {
                    s.transcript.partial.clear();
                    None
                } else {
                    s.last_word = Some(Instant::now());
                    s.transcript.push(&text)
                };
                let live = s.transcript.live();
                if let Some(e) = &s.events {
                    if let Some(p) = piece {
                        (e.phrase)(p);
                    }
                    (e.live)(live);
                }
            }
            Ok(())
        }))?;

        let s = shared.clone();
        let completed = session.Completed(&TypedEventHandler::<
            SpeechContinuousRecognitionSession,
            SpeechContinuousRecognitionCompletedEventArgs,
        >::new(move |_, args| {
            let why = args
                .as_ref()
                .and_then(|a| a.Status().ok())
                .and_then(ended_because);
            finish(&s, why);
            Ok(())
        }))?;

        let started = session.StartAsync().and_then(|op| op.join());
        if let Err(e) = started {
            let _ = rec.RemoveHypothesisGenerated(hyp);
            let _ = session.RemoveResultGenerated(res);
            let _ = session.RemoveCompleted(completed);
            return Err(e);
        }
        Ok(Live {
            recognizer: rec,
            session,
            shared: shared.clone(),
            tokens: (hyp, res, completed),
        })
    }

    /// Report the end once (whichever of Completed / stop / cancel comes
    /// first). Words still being spoken count as said.
    fn finish(shared: &Arc<Mutex<Shared>>, why: Option<String>) {
        let (events, text, last) = {
            let mut s = shared.lock().expect("speech");
            let last = s.transcript.flush();
            (s.events.take(), s.transcript.text().to_string(), last)
        };
        if let Some(e) = events {
            if let Some(p) = last {
                (e.phrase)(p);
            }
            (e.live)(text.clone());
            (e.done)(text, why);
        }
        let live = LIVE.lock().expect("speech").take();
        if let Some(live) = live {
            detach(&live);
            // Windows may take a few seconds to wind down; don't wait for it.
            std::thread::spawn(move || {
                init_com();
                let _ = live.session.CancelAsync().and_then(|op| op.join());
            });
        }
    }

    fn detach(live: &Live) {
        let _ = live.recognizer.RemoveHypothesisGenerated(live.tokens.0);
        let _ = live.session.RemoveResultGenerated(live.tokens.1);
        let _ = live.session.RemoveCompleted(live.tokens.2);
    }

    /// Stop listening now, keeping what was heard.
    pub fn stop() {
        let shared = LIVE
            .lock()
            .expect("speech")
            .as_ref()
            .map(|l| l.shared.clone());
        if let Some(shared) = shared {
            finish(&shared, None);
        }
    }

    /// Stop listening and drop what was heard.
    pub fn cancel() {
        // Release the lock first: cancelling fires Completed, which takes it.
        let live = LIVE.lock().expect("speech").take();
        if let Some(live) = live {
            live.shared.lock().expect("speech").events = None;
            detach(&live);
            let _ = live.session.CancelAsync().and_then(|op| op.join());
        }
    }
}

#[cfg(not(windows))]
mod imp {
    use super::*;
    pub fn prepare() -> Result<()> {
        Err(unsupported())
    }
    pub fn start(_events: Events) -> Result<()> {
        Err(unsupported())
    }
    pub fn stop() {}
    pub fn cancel() {}
    fn unsupported() -> wisp_core::WispError {
        wisp_core::WispError::new(
            wisp_core::ErrorCode::Unsupported,
            "Voice uses Windows speech recognition.",
        )
    }
}

pub use imp::{cancel, prepare, start, stop};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phrases_are_spaced_and_capitalised() {
        let mut t = Transcript {
            partial: "what's the".into(),
            ..Default::default()
        };
        assert_eq!(t.live(), "what's the");
        assert_eq!(
            t.push("what's the weather").as_deref(),
            Some("What's the weather")
        );
        t.partial = "in paris".into();
        assert_eq!(t.live(), "What's the weather in paris");
        assert_eq!(
            t.push(" in Paris tomorrow? ").as_deref(),
            Some(" in Paris tomorrow?")
        );
        assert_eq!(t.push("and london").as_deref(), Some(" And london"));
        assert_eq!(t.push("   "), None);
        assert_eq!(t.text(), "What's the weather in Paris tomorrow? And london");
        assert_eq!(t.live(), t.text());
    }

    #[test]
    fn noise_and_unfinished_words() {
        let mut t = Transcript::default();
        assert!(t.guess("open"));
        // The same guess again is noise, not more talking.
        assert!(!t.guess("open"));
        assert!(!t.guess("  "));
        assert!(t.guess("open spotify"));
        // Stopping mid-sentence keeps the words.
        assert_eq!(t.flush().as_deref(), Some("Open spotify"));
        assert_eq!(t.flush(), None);
        assert_eq!(t.text(), "Open spotify");
    }

    #[test]
    fn ends_after_a_pause_or_when_nothing_is_said() {
        let t0 = Instant::now();
        assert!(!should_end(t0, None, t0 + Duration::from_secs(3)));
        assert!(should_end(t0, None, t0 + START_SILENCE));
        let spoke = t0 + Duration::from_secs(2);
        assert!(!should_end(
            t0,
            Some(spoke),
            spoke + Duration::from_millis(800)
        ));
        assert!(should_end(t0, Some(spoke), spoke + END_SILENCE));
        // However much Windows thinks it hears, a session has a limit.
        let busy = t0 + MAX_SESSION;
        assert!(should_end(t0, Some(busy), busy));
    }
}
