//! Hold-to-talk detection for modifier-only chords (Ctrl+Win, Win+Alt).
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
}

pub const HOLD_DELAY: Duration = Duration::from_millis(250);

#[derive(Debug, Default)]
pub struct HoldDetector {
    ctrl: bool,
    alt: bool,
    shift: bool,
    win: bool,
    /// Chord candidate and when it began.
    candidate: Option<(Chord, Instant)>,
    active: Option<Chord>,
    /// Another key was pressed during this modifier session.
    spoiled: bool,
}

impl HoldDetector {
    pub fn new() -> Self {
        Self::default()
    }

    fn chord(&self) -> Option<Chord> {
        match (self.ctrl, self.alt, self.shift, self.win) {
            (true, false, false, true) => Some(Chord::Dictate),
            (false, true, false, true) => Some(Chord::Ask),
            _ => None,
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
                    self.spoiled = false;
                    self.candidate = None;
                    return self.active.take().map(HoldEvent::Stop);
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
