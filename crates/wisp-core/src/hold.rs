//! Hold-to-talk detection for modifier-only chords (Ctrl+Win and Win+Alt
//! by default; the user can pick other pairs).
//!
//! A pure state machine fed with key down/up events from a low-level
//! keyboard hook. It never consumes keys: it only reports when a chord has
//! been held on its own long enough, and when it is released. Pressing any
//! other key while the chord is down means "this is a normal shortcut"
//! (e.g. Ctrl+Win+D for virtual desktops) and cancels detection.

use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Mod {
    Ctrl,
    Alt,
    Shift,
    Win,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Mod(Mod),
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Chord {
    /// Ctrl + Win → dictation.
    Dictate,
    /// Win + Alt → ask by voice.
    Ask,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HoldEvent {
    Start(Chord),
    Stop(Chord),
    /// A started chord was turned into a normal shortcut; discard capture.
    Cancel(Chord),
    /// The chord was pressed on its own and let go before the hold delay.
    Tap(Chord),
}

pub const HOLD_DELAY: Duration = Duration::from_millis(250);

/// A set of held modifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Mods {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub win: bool,
}

/// The pairs offered for hold-to-talk (two modifiers, nothing else).
pub const HOLD_CHOICES: &[&str] = &[
    "ctrl+win",
    "win+alt",
    "ctrl+alt",
    "ctrl+shift",
    "alt+shift",
    "win+shift",
    "off",
];

/// "ctrl+win" → `Some(Some(mods))`; "off" → `Some(None)`; anything not
/// offered → `None`.
pub fn parse_chord(text: &str) -> Option<Option<Mods>> {
    let t = text.trim().to_ascii_lowercase();
    if !HOLD_CHOICES.contains(&t.as_str()) {
        return None;
    }
    if t == "off" {
        return Some(None);
    }
    let mut m = Mods::default();
    for part in t.split('+') {
        match part {
            "ctrl" => m.ctrl = true,
            "alt" => m.alt = true,
            "shift" => m.shift = true,
            "win" => m.win = true,
            _ => return None,
        }
    }
    Some(Some(m))
}

const CTRL_WIN: Mods = Mods {
    ctrl: true,
    alt: false,
    shift: false,
    win: true,
};
const WIN_ALT: Mods = Mods {
    ctrl: false,
    alt: true,
    shift: false,
    win: true,
};

#[derive(Debug)]
pub struct HoldDetector {
    ctrl: bool,
    alt: bool,
    shift: bool,
    win: bool,
    dictate: Option<Mods>,
    ask: Option<Mods>,
    /// Chord candidate and when it began.
    candidate: Option<(Chord, Instant)>,
    active: Option<Chord>,
    /// Another key was pressed during this modifier session.
    spoiled: bool,
}

impl Default for HoldDetector {
    fn default() -> Self {
        Self {
            ctrl: false,
            alt: false,
            shift: false,
            win: false,
            dictate: Some(CTRL_WIN),
            ask: Some(WIN_ALT),
            candidate: None,
            active: None,
            spoiled: false,
        }
    }
}

impl HoldDetector {
    pub fn new() -> Self {
        Self::default()
    }

    /// Choose which modifier pairs mean dictate and ask (None turns one off).
    pub fn set_chords(&mut self, dictate: Option<Mods>, ask: Option<Mods>) {
        self.dictate = dictate;
        self.ask = if ask == dictate { None } else { ask };
        self.candidate = None;
        self.active = None;
    }

    /// The modifiers of a chord (to know whether it involves the Win key).
    pub fn mods(&self, chord: Chord) -> Option<Mods> {
        match chord {
            Chord::Dictate => self.dictate,
            Chord::Ask => self.ask,
        }
    }

    /// A chord is being held but hasn't started yet (needs `tick`).
    pub fn waiting(&self) -> bool {
        self.candidate.is_some()
    }

    /// The chord being held, before it starts.
    pub fn candidate(&self) -> Option<Chord> {
        self.candidate.map(|(c, _)| c)
    }

    fn chord(&self) -> Option<Chord> {
        let held = Some(Mods {
            ctrl: self.ctrl,
            alt: self.alt,
            shift: self.shift,
            win: self.win,
        });
        if self.dictate.is_some() && held == self.dictate {
            Some(Chord::Dictate)
        } else if self.ask.is_some() && held == self.ask {
            Some(Chord::Ask)
        } else {
            None
        }
    }

    fn any_mod(&self) -> bool {
        self.ctrl || self.alt || self.shift || self.win
    }

    /// Feed a key event. Returns an event when something changes.
    pub fn key(&mut self, key: Key, down: bool, now: Instant) -> Option<HoldEvent> {
        match key {
            Key::Other => {
                if down && self.any_mod() {
                    self.spoiled = true;
                    self.candidate = None;
                    if let Some(c) = self.active.take() {
                        return Some(HoldEvent::Cancel(c));
                    }
                }
                None
            }
            Key::Mod(m) => {
                let slot = match m {
                    Mod::Ctrl => &mut self.ctrl,
                    Mod::Alt => &mut self.alt,
                    Mod::Shift => &mut self.shift,
                    Mod::Win => &mut self.win,
                };
                if *slot == down {
                    // Auto-repeat of a held modifier.
                    return None;
                }
                *slot = down;
                if !self.any_mod() {
                    let tap = match (self.candidate.take(), self.spoiled, self.active) {
                        (Some((c, _)), false, None) => Some(HoldEvent::Tap(c)),
                        _ => None,
                    };
                    self.spoiled = false;
                    return self.active.take().map(HoldEvent::Stop).or(tap);
                }
                // Letting go of a chord that was pressed on its own (and
                // never became a hold) is a tap.
                if !down && self.active.is_none() && !self.spoiled {
                    if let Some((c, _)) = self.candidate.take() {
                        self.spoiled = true;
                        return Some(HoldEvent::Tap(c));
                    }
                }
                if let Some(active) = self.active {
                    if self.chord() != Some(active) && down {
                        // An extra modifier joined: treat as a normal shortcut.
                        self.active = None;
                        self.spoiled = true;
                        return Some(HoldEvent::Cancel(active));
                    }
                    if !down {
                        // Releasing either key of the chord ends the hold.
                        self.active = None;
                        self.spoiled = true;
                        return Some(HoldEvent::Stop(active));
                    }
                    return None;
                }
                self.candidate = match (self.chord(), self.spoiled) {
                    (Some(c), false) => Some((c, now)),
                    _ => None,
                };
                None
            }
        }
    }

    /// Call periodically (e.g. every 50 ms) to promote a held candidate.
    pub fn tick(&mut self, now: Instant) -> Option<HoldEvent> {
        let (chord, since) = self.candidate?;
        if self.active.is_none() && !self.spoiled && now.duration_since(since) >= HOLD_DELAY {
            self.candidate = None;
            self.active = Some(chord);
            return Some(HoldEvent::Start(chord));
        }
        None
    }

    pub fn active(&self) -> Option<Chord> {
        self.active
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chords_can_be_changed() {
        assert_eq!(
            parse_chord("Ctrl+Alt").unwrap(),
            Some(Mods {
                ctrl: true,
                alt: true,
                ..Default::default()
            })
        );
        assert_eq!(parse_chord("off").unwrap(), None);
        assert!(parse_chord("ctrl+x").is_none());
        assert!(parse_chord("ctrl+alt+shift").is_none());
        let t0 = Instant::now();
        let mut d = HoldDetector::new();
        d.set_chords(parse_chord("ctrl+alt").unwrap(), None);
        d.key(Key::Mod(Mod::Ctrl), true, t0);
        d.key(Key::Mod(Mod::Alt), true, t0);
        assert!(d.waiting());
        assert_eq!(
            d.tick(t0 + HOLD_DELAY),
            Some(HoldEvent::Start(Chord::Dictate))
        );
        assert_eq!(d.mods(Chord::Dictate).map(|m| m.win), Some(false));
        // The old Ctrl+Win chord no longer does anything.
        let mut d2 = HoldDetector::new();
        d2.set_chords(parse_chord("ctrl+alt").unwrap(), None);
        d2.key(Key::Mod(Mod::Ctrl), true, t0);
        d2.key(Key::Mod(Mod::Win), true, t0);
        assert!(!d2.waiting());
        assert_eq!(d2.tick(t0 + HOLD_DELAY), None);
    }

    fn at(ms: u64, base: Instant) -> Instant {
        base + Duration::from_millis(ms)
    }

    #[test]
    fn hold_ctrl_win_starts_and_release_stops() {
        let t0 = Instant::now();
        let mut d = HoldDetector::new();
        assert_eq!(d.key(Key::Mod(Mod::Ctrl), true, at(0, t0)), None);
        assert_eq!(d.key(Key::Mod(Mod::Win), true, at(20, t0)), None);
        assert_eq!(d.tick(at(100, t0)), None, "not held long enough yet");
        assert_eq!(d.tick(at(300, t0)), Some(HoldEvent::Start(Chord::Dictate)));
        assert_eq!(d.tick(at(400, t0)), None, "starts once");
        // Auto-repeat is ignored.
        assert_eq!(d.key(Key::Mod(Mod::Win), true, at(450, t0)), None);
        assert_eq!(
            d.key(Key::Mod(Mod::Win), false, at(900, t0)),
            Some(HoldEvent::Stop(Chord::Dictate))
        );
        assert_eq!(d.key(Key::Mod(Mod::Ctrl), false, at(910, t0)), None);
        assert_eq!(d.active(), None);
    }

    #[test]
    fn a_quick_press_on_its_own_is_a_tap() {
        let t0 = Instant::now();
        let mut d = HoldDetector::new();
        d.key(Key::Mod(Mod::Ctrl), true, at(0, t0));
        d.key(Key::Mod(Mod::Win), true, at(20, t0));
        assert_eq!(d.candidate(), Some(Chord::Dictate));
        assert_eq!(
            d.key(Key::Mod(Mod::Win), false, at(120, t0)),
            Some(HoldEvent::Tap(Chord::Dictate))
        );
        assert_eq!(d.key(Key::Mod(Mod::Ctrl), false, at(130, t0)), None);
        assert_eq!(d.tick(at(500, t0)), None);
        // Both keys up at once still counts.
        d.key(Key::Mod(Mod::Alt), true, at(1000, t0));
        d.key(Key::Mod(Mod::Win), true, at(1000, t0));
        d.key(Key::Mod(Mod::Alt), false, at(1100, t0));
        // (Alt went up first: that's the tap.)
        assert_eq!(d.key(Key::Mod(Mod::Win), false, at(1100, t0)), None);
    }

    #[test]
    fn no_tap_after_a_shortcut_or_a_single_modifier() {
        let t0 = Instant::now();
        let mut d = HoldDetector::new();
        d.key(Key::Mod(Mod::Ctrl), true, at(0, t0));
        d.key(Key::Mod(Mod::Win), true, at(0, t0));
        d.key(Key::Other, true, at(50, t0)); // Ctrl+Win+Right
        assert_eq!(d.key(Key::Mod(Mod::Win), false, at(100, t0)), None);
        assert_eq!(d.key(Key::Mod(Mod::Ctrl), false, at(110, t0)), None);
        // Ctrl alone, then released.
        d.key(Key::Mod(Mod::Ctrl), true, at(200, t0));
        assert_eq!(d.key(Key::Mod(Mod::Ctrl), false, at(260, t0)), None);
        // A held chord ends with Stop, not Tap.
        d.key(Key::Mod(Mod::Ctrl), true, at(300, t0));
        d.key(Key::Mod(Mod::Win), true, at(300, t0));
        d.tick(at(600, t0));
        assert_eq!(
            d.key(Key::Mod(Mod::Ctrl), false, at(700, t0)),
            Some(HoldEvent::Stop(Chord::Dictate))
        );
    }

    #[test]
    fn win_alt_is_ask() {
        let t0 = Instant::now();
        let mut d = HoldDetector::new();
        d.key(Key::Mod(Mod::Alt), true, at(0, t0));
        d.key(Key::Mod(Mod::Win), true, at(10, t0));
        assert_eq!(d.tick(at(300, t0)), Some(HoldEvent::Start(Chord::Ask)));
    }

    #[test]
    fn other_key_before_delay_is_a_normal_shortcut() {
        let t0 = Instant::now();
        let mut d = HoldDetector::new();
        d.key(Key::Mod(Mod::Ctrl), true, at(0, t0));
        d.key(Key::Mod(Mod::Win), true, at(10, t0));
        d.key(Key::Other, true, at(50, t0)); // Ctrl+Win+D (virtual desktop)
        assert_eq!(d.tick(at(400, t0)), None);
        // Still spoiled until all modifiers are released.
        d.key(Key::Other, false, at(60, t0));
        assert_eq!(d.tick(at(800, t0)), None);
        d.key(Key::Mod(Mod::Win), false, at(900, t0));
        d.key(Key::Mod(Mod::Ctrl), false, at(910, t0));
        d.key(Key::Mod(Mod::Ctrl), true, at(1000, t0));
        d.key(Key::Mod(Mod::Win), true, at(1010, t0));
        assert_eq!(d.tick(at(1300, t0)), Some(HoldEvent::Start(Chord::Dictate)));
    }

    #[test]
    fn other_key_after_start_cancels() {
        let t0 = Instant::now();
        let mut d = HoldDetector::new();
        d.key(Key::Mod(Mod::Ctrl), true, at(0, t0));
        d.key(Key::Mod(Mod::Win), true, at(0, t0));
        d.tick(at(300, t0));
        assert_eq!(
            d.key(Key::Other, true, at(350, t0)),
            Some(HoldEvent::Cancel(Chord::Dictate))
        );
        assert_eq!(d.key(Key::Mod(Mod::Ctrl), false, at(400, t0)), None);
    }

    #[test]
    fn extra_modifier_cancels_and_shift_combos_never_start() {
        let t0 = Instant::now();
        let mut d = HoldDetector::new();
        d.key(Key::Mod(Mod::Ctrl), true, at(0, t0));
        d.key(Key::Mod(Mod::Win), true, at(0, t0));
        d.tick(at(300, t0));
        assert_eq!(
            d.key(Key::Mod(Mod::Shift), true, at(320, t0)),
            Some(HoldEvent::Cancel(Chord::Dictate))
        );
        let mut d = HoldDetector::new();
        d.key(Key::Mod(Mod::Ctrl), true, at(0, t0));
        d.key(Key::Mod(Mod::Shift), true, at(0, t0));
        d.key(Key::Mod(Mod::Win), true, at(0, t0));
        assert_eq!(d.tick(at(500, t0)), None);
    }
}
