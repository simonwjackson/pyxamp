//! The Jellyfin queue: occurrences, the current one, repeat, and shuffle as a
//! mode that can be undone. Pure; the actor drives the engine from it.
//!
//! Jellyfin keeps no queue for an API client, so the connector owns it here
//! (ADR 0001, Jellyfin exception). It is the only copy.
use super::{api::BaseItem, library};
use crate::protocol::{Failure, Item, QueueItem, QueueStatus, Repeat, Track};

#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub occurrence: String,
    /// The Jellyfin item id.
    pub id: String,
    pub item: Item,
    pub track: Track,
    pub album_key: Option<String>,
    pub status: QueueStatus,
}

/// A track that can enter the queue (no occurrence yet).
#[derive(Clone, Debug, PartialEq)]
pub struct NewEntry {
    id: String,
    item: Item,
    track: Track,
    album_key: Option<String>,
}

impl NewEntry {
    /// Audio items only; videos are handed off and never queued.
    pub fn from_item(item: &BaseItem) -> Option<Self> {
        if !library::is_audio(item) {
            return None;
        }
        let wire = library::to_item(item)?;
        let track = Track {
            title: wire.title.clone(),
            // `Track.artist` is required on the wire; an unknown artist is empty.
            artist: wire.subtitle.clone().unwrap_or_default(),
            id: Some(wire.id.clone()),
            album: wire.album.clone(),
            duration_ms: wire.duration_ms,
            ..Track::default()
        };
        Some(Self {
            id: item.id.clone(),
            album_key: item.album_id.clone().filter(|a| !a.is_empty()),
            item: wire,
            track,
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Queue {
    entries: Vec<Entry>,
    current: Option<usize>,
    /// The order before shuffle, as occurrences; `Some` while shuffle is on.
    unshuffled: Option<Vec<String>>,
    pub repeat: Repeat,
    serial: u64,
}

impl Default for Queue {
    fn default() -> Self {
        Self {
            entries: vec![],
            current: None,
            unshuffled: None,
            repeat: Repeat::Off,
            serial: 0,
        }
    }
}

fn not_found() -> Failure {
    Failure::new("notFound", "no such queue occurrence")
}

impl Queue {
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn shuffle(&self) -> bool {
        self.unshuffled.is_some()
    }

    pub fn current(&self) -> Option<&Entry> {
        self.current.map(|i| &self.entries[i])
    }

    pub fn entry(&self, index: usize) -> Option<&Entry> {
        self.entries.get(index)
    }

    pub fn current_index(&self) -> Option<usize> {
        self.current
    }

    pub fn index_of(&self, occurrence: &str) -> Option<usize> {
        self.entries.iter().position(|e| e.occurrence == occurrence)
    }

    fn admit(&mut self, new: Vec<NewEntry>) -> Vec<Entry> {
        new.into_iter()
            .map(|n| {
                self.serial += 1;
                Entry {
                    occurrence: format!("jf{}", self.serial),
                    id: n.id,
                    item: n.item,
                    track: n.track,
                    album_key: n.album_key,
                    status: QueueStatus::Pending,
                }
            })
            .collect()
    }

    /// Replace everything; the first new entry becomes current.
    pub fn replace(&mut self, new: Vec<NewEntry>) {
        let entries = self.admit(new);
        self.entries = entries;
        self.current = None;
        if self.unshuffled.is_some() {
            self.unshuffled = Some(self.occurrences());
        }
        if !self.entries.is_empty() {
            self.set_current(0, false);
        }
    }

    /// After the current occurrence (at the start when none is current).
    pub fn insert_next(&mut self, new: Vec<NewEntry>) -> usize {
        let at = self.current.map_or(0, |i| i + 1);
        let entries = self.admit(new);
        if let Some(order) = &mut self.unshuffled {
            let after = self
                .current
                .and_then(|i| order.iter().position(|o| *o == self.entries[i].occurrence))
                .map_or(0, |p| p + 1);
            order.splice(after..after, entries.iter().map(|e| e.occurrence.clone()));
        }
        self.entries.splice(at..at, entries);
        at
    }

    pub fn append(&mut self, new: Vec<NewEntry>) -> usize {
        let at = self.entries.len();
        let entries = self.admit(new);
        if let Some(order) = &mut self.unshuffled {
            order.extend(entries.iter().map(|e| e.occurrence.clone()));
        }
        self.entries.extend(entries);
        at
    }

    /// Make `index` current. `finished` marks the old current played.
    pub fn set_current(&mut self, index: usize, finished: bool) {
        if let Some(old) = self.current
            && let Some(entry) = self.entries.get_mut(old)
            && matches!(entry.status, QueueStatus::Current | QueueStatus::Retrying)
        {
            entry.status = if finished {
                QueueStatus::Played
            } else {
                QueueStatus::Pending
            };
        }
        self.current = Some(index);
        self.entries[index].status = QueueStatus::Current;
    }

    pub fn set_current_status(&mut self, status: QueueStatus) {
        if let Some(i) = self.current {
            self.entries[i].status = status;
        }
    }

    /// Clear the current occurrence (end of the queue), marking it played.
    pub fn finish(&mut self) {
        if let Some(i) = self.current.take()
            && self.entries[i].status == QueueStatus::Current
        {
            self.entries[i].status = QueueStatus::Played;
        }
    }

    pub fn jump(&mut self, occurrence: &str) -> Result<usize, Failure> {
        let index = self.index_of(occurrence).ok_or_else(not_found)?;
        self.set_current(index, false);
        Ok(index)
    }

    /// Remove an occurrence. Returns true when it was the current one; then the
    /// entry that followed it (if any) is current.
    pub fn remove(&mut self, occurrence: &str) -> Result<bool, Failure> {
        let index = self.index_of(occurrence).ok_or_else(not_found)?;
        self.entries.remove(index);
        if let Some(order) = &mut self.unshuffled {
            order.retain(|o| o != occurrence);
        }
        let was_current = self.current == Some(index);
        self.current = match self.current {
            Some(c) if c > index => Some(c - 1),
            Some(c) if c == index => None,
            other => other,
        };
        if was_current && index < self.entries.len() {
            self.set_current(index, false);
        }
        Ok(was_current)
    }

    /// The entry after the current one. `automatic` is the end of a track,
    /// where repeat one plays the same entry again.
    pub fn peek_next(&self, automatic: bool) -> Option<usize> {
        let len = self.entries.len();
        let Some(i) = self.current else {
            return (len > 0).then_some(0);
        };
        if automatic && self.repeat == Repeat::One {
            return Some(i);
        }
        if i + 1 < len {
            Some(i + 1)
        } else if self.repeat == Repeat::All && len > 0 {
            Some(0)
        } else {
            None
        }
    }

    pub fn peek_previous(&self) -> Option<usize> {
        let i = self.current?;
        if i > 0 {
            Some(i - 1)
        } else if self.repeat == Repeat::All {
            Some(self.entries.len() - 1)
        } else {
            None
        }
    }

    fn occurrences(&self) -> Vec<String> {
        self.entries.iter().map(|e| e.occurrence.clone()).collect()
    }

    /// Shuffle the entries after the current one; off restores the order.
    pub fn set_shuffle(&mut self, enabled: bool, seed: u64) {
        let current = self.current().map(|e| e.occurrence.clone());
        match (enabled, self.unshuffled.take()) {
            (true, None) => {
                self.unshuffled = Some(self.occurrences());
                let from = self.current.map_or(0, |i| i + 1);
                let mut state = seed | 1;
                let tail = &mut self.entries[from..];
                for i in (1..tail.len()).rev() {
                    // xorshift64: enough for play order, no dependency.
                    state ^= state << 13;
                    state ^= state >> 7;
                    state ^= state << 17;
                    tail.swap(i, (state % (i as u64 + 1)) as usize);
                }
            }
            (true, order @ Some(_)) => self.unshuffled = order,
            (false, Some(order)) => {
                self.entries.sort_by_key(|e| {
                    order
                        .iter()
                        .position(|o| *o == e.occurrence)
                        .unwrap_or(usize::MAX)
                });
            }
            (false, None) => {}
        }
        self.current = current.and_then(|o| self.index_of(&o));
    }

    pub fn wire(&self) -> Vec<QueueItem> {
        self.entries
            .iter()
            .map(|e| QueueItem {
                occurrence: e.occurrence.clone(),
                item: e.item.clone(),
                album_key: e.album_key.clone(),
                status: e.status,
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tracks(names: &[&str]) -> Vec<NewEntry> {
        names
            .iter()
            .map(|n| {
                NewEntry::from_item(&BaseItem {
                    id: format!("id{n}"),
                    name: Some((*n).into()),
                    kind: Some("Audio".into()),
                    album_id: Some(format!("album{}", &n[..1])),
                    ..Default::default()
                })
                .unwrap()
            })
            .collect()
    }

    fn titles(queue: &Queue) -> Vec<String> {
        queue.wire().iter().map(|q| q.item.title.clone()).collect()
    }

    #[test]
    fn only_audio_enters_the_queue() {
        let video = BaseItem {
            id: "v".into(),
            name: Some("Clip".into()),
            kind: Some("Video".into()),
            ..Default::default()
        };
        assert!(NewEntry::from_item(&video).is_none());
        let entry = &tracks(&["a1"])[0];
        assert_eq!(entry.track.artist, "", "an unknown artist stays empty");
        assert_eq!(entry.album_key.as_deref(), Some("albuma"));
    }

    #[test]
    fn occurrences_insert_next_and_last_and_the_same_track_twice() {
        let mut queue = Queue::default();
        queue.replace(tracks(&["a1", "a2"]));
        queue.append(tracks(&["b1"]));
        queue.insert_next(tracks(&["a2"]));
        assert_eq!(titles(&queue), ["a1", "a2", "a2", "b1"]);
        let wire = queue.wire();
        assert_ne!(wire[1].occurrence, wire[2].occurrence);
        assert_eq!(wire[0].status, QueueStatus::Current);
        assert_eq!(wire[0].album_key, wire[1].album_key);
        assert_ne!(wire[2].album_key, wire[3].album_key);
    }

    #[test]
    fn next_previous_and_repeat_modes() {
        let mut queue = Queue::default();
        queue.replace(tracks(&["a1", "a2"]));
        assert_eq!(queue.peek_previous(), None);
        assert_eq!(queue.peek_next(true), Some(1));
        queue.set_current(1, true);
        assert_eq!(queue.wire()[0].status, QueueStatus::Played);
        assert_eq!(queue.peek_next(true), None);
        queue.repeat = Repeat::All;
        assert_eq!(queue.peek_next(true), Some(0));
        assert_eq!(queue.peek_previous(), Some(0));
        queue.repeat = Repeat::One;
        assert_eq!(
            queue.peek_next(true),
            Some(1),
            "repeat one replays at the end"
        );
        assert_eq!(queue.peek_next(false), None, "Next still moves on");
        queue.finish();
        assert_eq!(queue.current(), None);
        assert_eq!(queue.wire()[1].status, QueueStatus::Played);
    }

    #[test]
    fn remove_and_jump() {
        let mut queue = Queue::default();
        queue.replace(tracks(&["a1", "a2", "a3"]));
        let occ = |q: &Queue, i: usize| q.wire()[i].occurrence.clone();
        let second = occ(&queue, 1);
        assert_eq!(queue.jump(&second).unwrap(), 1);
        assert_eq!(queue.wire()[0].status, QueueStatus::Pending);
        assert!(queue.remove(&second).unwrap(), "removing the current one");
        assert_eq!(queue.current().unwrap().item.title, "a3");
        let first = occ(&queue, 0);
        assert!(!queue.remove(&first).unwrap());
        assert_eq!(queue.current_index(), Some(0));
        assert!(queue.jump("nope").is_err());
    }

    #[test]
    fn shuffle_keeps_the_current_entry_and_off_restores_the_order() {
        let names = ["a1", "a2", "a3", "a4", "a5", "a6", "a7", "a8"];
        let mut queue = Queue::default();
        queue.replace(tracks(&names));
        queue.set_current(2, false);
        queue.set_shuffle(true, 0x5eed);
        assert!(queue.shuffle());
        let shuffled = titles(&queue);
        assert_eq!(&shuffled[..3], ["a1", "a2", "a3"]);
        assert_ne!(shuffled, names, "the tail moved");
        let mut sorted = shuffled.clone();
        sorted.sort();
        assert_eq!(sorted, names);
        queue.append(tracks(&["b1"]));
        queue.insert_next(tracks(&["c1"]));
        queue.set_shuffle(false, 0);
        assert_eq!(
            titles(&queue),
            ["a1", "a2", "a3", "c1", "a4", "a5", "a6", "a7", "a8", "b1"]
        );
        assert_eq!(queue.current().unwrap().item.title, "a3");
    }
}
