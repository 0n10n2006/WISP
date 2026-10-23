//! Instant voice: Windows' built-in speech recognizer turns speech into text
//! while the user talks, so words appear live and the answer can start the
//! moment they stop. The webview records the same audio in parallel; if
//! Windows can't make it out (or isn't available), that recording goes to
//! Gemini instead. Windows dictation uses Microsoft's online speech service;
//! the Voice settings say so and offer Gemini instead.

use serde::Serialize;
use wisp_core::Result;

/// Whether the Windows recognizer can be used, and if not, why.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SpeechStatus {
    pub available: bool,
    pub reason: Option<String>,
    /// The Windows settings page that fixes `reason`, when there is one.
    pub fix_uri: Option<String>,
}

/// What was heard, and how sure Windows is.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Heard {
    pub text: String,
    /// "high" | "medium" | "low" | "rejected" (the least sure phrase).
    pub confidence: String,
}

/// Join recognised phrases and keep the weakest confidence.
#[derive(Default)]
#[cfg_attr(not(windows), allow(dead_code))]
pub struct Transcript {
    phrases: Vec<String>,
    worst: u8,
    pub partial: String,
}

#[cfg_attr(not(windows), allow(dead_code))]
impl Transcript {
    /// Add a finished phrase. `level`: 0 high … 3 rejected.
    pub fn push(&mut self, text: &str, level: u8) {
        let t = text.trim();
        if !t.is_empty() {
            self.phrases.push(t.to_string());
            self.worst = self.worst.max(level.min(3));
        } else if level >= 3 {
            self.worst = 3;
        }
        self.partial.clear();
    }

    /// What to show while the user is still speaking.
    pub fn live(&self) -> String {
        let mut s = self.phrases.join(" ");
        if !self.partial.trim().is_empty() {
            if !s.is_empty() {
                s.push(' ');
            }
            s.push_str(self.partial.trim());
        }
        s
    }

    pub fn finish(&self) -> Heard {
        let text = self.phrases.join(" ");
        Heard {
            confidence: if text.is_empty() {
                "rejected".into()
            } else {
                ["high", "medium", "low", "rejected"][self.worst as usize].into()
            },
            text,
        }
    }
}

#[cfg(windows)]
mod imp {
    use super::*;
    use std::sync::{mpsc, Arc, Mutex};
    use std::time::Duration;
    use windows::core::{Interface, HSTRING};
    use windows::Foundation::TypedEventHandler;
    use windows::Globalization::Language;
    use windows::Media::SpeechRecognition::{
        ISpeechRecognitionConstraint, SpeechContinuousRecognitionCompletedEventArgs,
        SpeechContinuousRecognitionResultGeneratedEventArgs, SpeechContinuousRecognitionSession,
        SpeechRecognitionConfidence, SpeechRecognitionHypothesisGeneratedEventArgs,
        SpeechRecognitionResultStatus, SpeechRecognitionScenario, SpeechRecognitionTopicConstraint,
        SpeechRecognizer,
    };
    use wisp_core::{ErrorCode, WispError};

    /// HRESULT Windows returns when online speech recognition is turned off.
    const PRIVACY_DECLINED: i32 = 0x8004_5509_u32 as i32;

    struct Live {
        recognizer: SpeechRecognizer,
        session: SpeechContinuousRecognitionSession,
        transcript: Arc<Mutex<Transcript>>,
        done: mpsc::Receiver<()>,
        tokens: (i64, i64, i64),
    }

    /// A compiled recognizer, kept warm between uses (no audio until started).
    static READY: Mutex<Option<(String, SpeechRecognizer)>> = Mutex::new(None);
    static LIVE: Mutex<Option<Live>> = Mutex::new(None);

    fn init_com() {
        unsafe {
            use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        }
    }

    fn explain(e: &windows::core::Error) -> WispError {
        if e.code().0 == PRIVACY_DECLINED {
            WispError::new(
                ErrorCode::PermissionDenied,
                "Windows speech recognition needs \"Online speech recognition\" turned on in Settings → Privacy → Speech.",
            )
        } else {
            WispError::new(
                ErrorCode::Unsupported,
                format!("Windows speech recognition isn't available ({}).", e.code()),
            )
        }
    }

    fn compile(language: &str) -> Result<SpeechRecognizer> {
        let mut ready = READY.lock().expect("speech");
        if let Some((lang, r)) = ready.as_ref() {
            if lang == language {
                return Ok(r.clone());
            }
        }
        let rec = if language.is_empty() {
            SpeechRecognizer::new()
        } else {
            Language::CreateLanguage(&HSTRING::from(language))
                .and_then(|l| SpeechRecognizer::Create(&l))
                .or_else(|_| SpeechRecognizer::new())
        }
        .map_err(|e| explain(&e))?;
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
                    "Windows has no speech recognition for this language. Add it in Settings → Time & language → Speech.".to_string()
                } else {
                    format!("Windows speech recognition isn't ready ({}).", status.0)
                },
            ));
        }
        // Wisp decides when the user has finished (hold release or its own
        // pause detection); Windows shouldn't stop early.
        if let Ok(s) = rec.ContinuousRecognitionSession() {
            let _ = s.SetAutoStopSilenceTimeout(Duration::from_secs(120).into());
        }
        *ready = Some((language.to_string(), rec.clone()));
        Ok(rec)
    }

    pub fn status(language: &str) -> SpeechStatus {
        init_com();
        match compile(language) {
            Ok(_) => SpeechStatus {
                available: true,
                reason: None,
                fix_uri: None,
            },
            Err(e) => SpeechStatus {
                available: false,
                fix_uri: Some(
                    if e.code == ErrorCode::PermissionDenied {
                        "ms-settings:privacy-speech"
                    } else {
                        "ms-settings:speech"
                    }
                    .into(),
                ),
                reason: Some(e.message),
            },
        }
    }

    fn level(c: SpeechRecognitionConfidence) -> u8 {
        match c {
            SpeechRecognitionConfidence::High => 0,
            SpeechRecognitionConfidence::Medium => 1,
            SpeechRecognitionConfidence::Low => 2,
            _ => 3,
        }
    }

    pub fn start(language: &str, on_text: impl Fn(String) + Send + Sync + 'static) -> Result<()> {
        init_com();
        cancel();
        let rec = compile(language)?;
        let session = rec
            .ContinuousRecognitionSession()
            .map_err(|e| explain(&e))?;
        let transcript = Arc::new(Mutex::new(Transcript::default()));
        let on_text = Arc::new(on_text);
        let (tx, done) = mpsc::channel();

        let t = transcript.clone();
        let f = on_text.clone();
        let hyp = rec
            .HypothesisGenerated(&TypedEventHandler::<
                SpeechRecognizer,
                SpeechRecognitionHypothesisGeneratedEventArgs,
            >::new(move |_, args| {
                if let Some(text) = args
                    .as_ref()
                    .and_then(|a| a.Hypothesis().ok())
                    .and_then(|h| h.Text().ok())
                {
                    let live = {
                        let mut t = t.lock().expect("speech");
                        t.partial = text.to_string();
                        t.live()
                    };
                    f(live);
                }
                Ok(())
            }))
            .map_err(|e| explain(&e))?;

        let t = transcript.clone();
        let f = on_text.clone();
        let res = session
            .ResultGenerated(&TypedEventHandler::<
                SpeechContinuousRecognitionSession,
                SpeechContinuousRecognitionResultGeneratedEventArgs,
            >::new(move |_, args| {
                if let Some(r) = args.as_ref().and_then(|a| a.Result().ok()) {
                    let text = r.Text().map(|s| s.to_string()).unwrap_or_default();
                    let lvl = r.Confidence().map(level).unwrap_or(3);
                    let live = {
                        let mut t = t.lock().expect("speech");
                        t.push(&text, lvl);
                        t.live()
                    };
                    f(live);
                }
                Ok(())
            }))
            .map_err(|e| explain(&e))?;

        let tx = Mutex::new(tx);
        let completed = session
            .Completed(&TypedEventHandler::<
                SpeechContinuousRecognitionSession,
                SpeechContinuousRecognitionCompletedEventArgs,
            >::new(move |_, _| {
                let _ = tx.lock().expect("speech").send(());
                Ok(())
            }))
            .map_err(|e| explain(&e))?;

        session
            .StartAsync()
            .and_then(|op| op.join())
            .map_err(|e| explain(&e))?;
        *LIVE.lock().expect("speech") = Some(Live {
            recognizer: rec,
            session,
            transcript,
            done,
            tokens: (hyp, res, completed),
        });
        Ok(())
    }

    fn detach(live: &Live) {
        let _ = live.recognizer.RemoveHypothesisGenerated(live.tokens.0);
        let _ = live.session.RemoveResultGenerated(live.tokens.1);
        let _ = live.session.RemoveCompleted(live.tokens.2);
    }

    pub fn stop() -> Result<Heard> {
        init_com();
        let Some(live) = LIVE.lock().expect("speech").take() else {
            return Ok(Transcript::default().finish());
        };
        // Stopping flushes the last phrase; wait briefly for it.
        let stopped = live.session.StopAsync().and_then(|op| op.join());
        if stopped.is_ok() {
            let _ = live.done.recv_timeout(Duration::from_millis(1500));
        }
        detach(&live);
        let heard = live.transcript.lock().expect("speech").finish();
        Ok(heard)
    }

    pub fn cancel() {
        if let Some(live) = LIVE.lock().expect("speech").take() {
            let _ = live.session.CancelAsync().and_then(|op| op.join());
            detach(&live);
        }
    }
}

#[cfg(not(windows))]
mod imp {
    use super::*;
    pub fn status(_language: &str) -> SpeechStatus {
        SpeechStatus {
            available: false,
            reason: Some("Instant voice uses Windows speech recognition.".into()),
            fix_uri: None,
        }
    }
    pub fn start(_language: &str, _on_text: impl Fn(String) + Send + Sync + 'static) -> Result<()> {
        Err(wisp_core::WispError::new(
            wisp_core::ErrorCode::Unsupported,
            "Instant voice uses Windows speech recognition.",
        ))
    }
    pub fn stop() -> Result<Heard> {
        Ok(Transcript::default().finish())
    }
    pub fn cancel() {}
}

pub use imp::{cancel, start, status, stop};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transcript_joins_phrases_and_keeps_the_weakest_confidence() {
        let mut t = Transcript {
            partial: "what's the".into(),
            ..Default::default()
        };
        assert_eq!(t.live(), "what's the");
        t.push("What's the weather", 0);
        t.partial = "in Paris".into();
        assert_eq!(t.live(), "What's the weather in Paris");
        t.push("in Paris tomorrow", 1);
        let h = t.finish();
        assert_eq!(h.text, "What's the weather in Paris tomorrow");
        assert_eq!(h.confidence, "medium");
        assert_eq!(Transcript::default().finish().confidence, "rejected");
        let mut r = Transcript::default();
        r.push("", 3);
        r.push("hello", 0);
        assert_eq!(r.finish().confidence, "rejected");
    }
}
