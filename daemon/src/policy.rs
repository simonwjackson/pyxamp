//! Pure follow and yield policy. Time is supplied by the caller, in milliseconds.
use crate::protocol::{Target, Transport};
use std::collections::BTreeMap;

pub fn is_self(bus_name: &str) -> bool {
    bus_name == "org.mpris.MediaPlayer2.pyxamp"
        || bus_name.starts_with("org.mpris.MediaPlayer2.pyxamp.")
}

#[derive(Default)]
pub struct FollowPolicy {
    players: BTreeMap<String, (Target, Transport, u64)>,
    serial: u64,
}
impl FollowPolicy {
    /// Polls may run before queued PropertiesChanged signals are consumed. Do not
    /// let a Playing snapshot swallow a later ordered transition signal.
    pub fn snapshot(&mut self, target: Target, transport: Transport, startup: bool) {
        let known = self
            .players
            .get(&target.bus_name)
            .is_some_and(|(old, _, _)| old == &target);
        if startup || !known || transport != Transport::Playing {
            self.observe(target, transport, startup);
        }
    }

    /// Startup observations share serial zero; lexically first bus name wins.
    pub fn observe(&mut self, target: Target, transport: Transport, startup: bool) {
        if is_self(&target.bus_name) {
            return;
        }
        let old = self.players.get(&target.bus_name);
        let transitioned = old
            .is_none_or(|(t, s, _)| t.owner != target.owner || *s != Transport::Playing)
            && transport == Transport::Playing;
        let order = if transitioned && !startup {
            self.serial += 1;
            self.serial
        } else {
            old.map_or(0, |(_, _, n)| *n)
        };
        self.players
            .insert(target.bus_name.clone(), (target, transport, order));
    }
    pub fn remove(&mut self, bus_name: &str) {
        self.players.remove(bus_name);
    }
    pub fn followed(&self) -> Option<&Target> {
        self.players
            .values()
            .filter(|(_, s, _)| *s == Transport::Playing)
            .max_by(|(a, _, x), (b, _, y)| x.cmp(y).then_with(|| b.bus_name.cmp(&a.bus_name)))
            .map(|(t, _, _)| t)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OwnPlayback {
    Idle,
    Paused { session: String },
    Playing { session: String },
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum YieldEffect {
    Pause,
    Resume,
}
#[derive(Clone, Debug, PartialEq, Eq)]
enum Interruption {
    None,
    Eligible { started_ms: u64, session: String },
    Cancelled,
}
pub struct YieldPolicy {
    interruption: Interruption,
    foreign_active: bool,
}
impl Default for YieldPolicy {
    fn default() -> Self {
        Self {
            interruption: Interruption::None,
            foreign_active: false,
        }
    }
}
impl YieldPolicy {
    pub fn cancel(&mut self) {
        if self.foreign_active {
            self.interruption = Interruption::Cancelled;
        }
    }
    pub fn update(
        &mut self,
        now_ms: u64,
        foreign_count: usize,
        own: &OwnPlayback,
    ) -> Option<YieldEffect> {
        let active = foreign_count > 0;
        if let Interruption::Eligible {
            started_ms,
            session,
        } = &self.interruption
        {
            let same_session = matches!(own, OwnPlayback::Paused { session: s } | OwnPlayback::Playing { session: s } if s == session);
            if now_ms.saturating_sub(*started_ms) > 120_000 || !same_session {
                self.interruption = Interruption::Cancelled;
            }
        }
        let effect = if active && !self.foreign_active {
            if let OwnPlayback::Playing { session } = own {
                self.interruption = Interruption::Eligible {
                    started_ms: now_ms,
                    session: session.clone(),
                };
                Some(YieldEffect::Pause)
            } else {
                self.interruption = Interruption::Cancelled;
                None
            }
        } else if !active && self.foreign_active {
            let resume = matches!((&self.interruption, own), (Interruption::Eligible { session, .. }, OwnPlayback::Paused { session: s }) if session == s);
            self.interruption = Interruption::None;
            resume.then_some(YieldEffect::Resume)
        } else {
            None
        };
        self.foreign_active = active;
        effect
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn target(name: &str, owner: &str) -> Target {
        Target {
            bus_name: format!("org.mpris.MediaPlayer2.{name}"),
            owner: owner.into(),
        }
    }
    #[test]
    fn follows_transitions_not_metadata_and_falls_back() {
        let mut p = FollowPolicy::default();
        let a = target("a", ":1.1");
        let b = target("b", ":1.2");
        p.observe(b.clone(), Transport::Playing, true);
        p.observe(a.clone(), Transport::Playing, true);
        assert_eq!(p.followed(), Some(&a));
        p.observe(a.clone(), Transport::Playing, false);
        assert_eq!(p.followed(), Some(&a));
        p.observe(b.clone(), Transport::Paused, false);
        p.observe(b.clone(), Transport::Playing, false);
        assert_eq!(p.followed(), Some(&b));
        p.observe(a.clone(), Transport::Playing, false);
        assert_eq!(p.followed(), Some(&b));
        p.remove(&b.bus_name);
        assert_eq!(p.followed(), Some(&a));
        p.observe(a.clone(), Transport::Stopped, false);
        assert_eq!(p.followed(), None);
        p.observe(target("pyxamp.other", ":1.3"), Transport::Playing, false);
        assert_eq!(p.followed(), None);
    }
    #[test]
    fn polling_cannot_swallow_or_reorder_queued_playing_transitions() {
        let mut policy = FollowPolicy::default();
        let a = target("a", ":1.1");
        let b = target("b", ":1.2");
        policy.snapshot(a.clone(), Transport::Paused, true);
        policy.snapshot(b.clone(), Transport::Paused, true);
        // Polling A then B must not invent their start order.
        policy.snapshot(a.clone(), Transport::Playing, false);
        policy.snapshot(b.clone(), Transport::Playing, false);
        assert!(policy.followed().is_none());
        policy.observe(b.clone(), Transport::Playing, false);
        policy.observe(a.clone(), Transport::Playing, false);
        assert_eq!(policy.followed(), Some(&a));
        policy.snapshot(b, Transport::Playing, false);
        assert_eq!(policy.followed(), Some(&a));
    }

    struct SessionPlayback {
        state: OwnPlayback,
        pauses: usize,
        resumes: usize,
    }
    impl SessionPlayback {
        fn apply(&mut self, effect: Option<YieldEffect>) {
            match effect {
                Some(YieldEffect::Pause) => {
                    self.pauses += 1;
                    self.state = OwnPlayback::Paused {
                        session: "s".into(),
                    };
                }
                Some(YieldEffect::Resume) => {
                    self.resumes += 1;
                    self.state = OwnPlayback::Playing {
                        session: "s".into(),
                    };
                }
                None => {}
            }
        }
    }
    #[test]
    fn overlapping_players_are_one_interruption_at_inclusive_limit() {
        let mut p = YieldPolicy::default();
        let mut playback = SessionPlayback {
            state: OwnPlayback::Playing {
                session: "s".into(),
            },
            pauses: 0,
            resumes: 0,
        };
        for (time, count) in [(10, 1), (20, 2), (1000, 1), (120_010, 0)] {
            playback.apply(p.update(time, count, &playback.state));
        }
        assert_eq!((playback.pauses, playback.resumes), (1, 1));
    }
    #[test]
    fn expiration_cancellation_and_session_change_prevent_resume() {
        for scenario in 0..4 {
            let mut p = YieldPolicy::default();
            let playing = OwnPlayback::Playing {
                session: "s".into(),
            };
            assert_eq!(p.update(0, 1, &playing), Some(YieldEffect::Pause));
            if scenario == 0 {
                p.cancel();
            }
            let own = OwnPlayback::Paused {
                session: if scenario == 1 { "new" } else { "s" }.into(),
            };
            if scenario == 3 {
                p.update(120_001, 1, &own);
            }
            assert_eq!(
                p.update(if scenario == 2 { 120_001 } else { 200 }, 0, &own),
                None
            );
        }
        let mut p = YieldPolicy::default();
        let paused = OwnPlayback::Paused {
            session: "s".into(),
        };
        assert_eq!(p.update(0, 1, &paused), None);
        assert_eq!(p.update(100, 0, &paused), None);
    }
}
