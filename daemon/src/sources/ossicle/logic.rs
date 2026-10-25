//! Pure rules of the Ossicle source: identities, settings, sponsor spans,
//! skip, retry and narration sync. No I/O, so each rule is unit-tested.
use super::wire::{
    Book, BookLocator, Episode, NarrationSegment, PodcastSettings, Settings, SponsorSegment,
    Timeline, TimelineSegment,
};
use crate::protocol::{Item, ItemKind, Progress, SkipIntervals, SourceId};

pub const GROUP_UP_NEXT: &str = "up-next";
pub const GROUP_SHOWS: &str = "shows";
pub const GROUP_BOOKS: &str = "books";

/// Two positions within this many seconds are the same listening moment (Ossicle's rule).
pub const SAME_MOMENT_S: u32 = 5;

pub use crate::sources::retry_delay;

// ---------- identities ----------

fn escape(part: &str) -> String {
    part.replace('%', "%25").replace(':', "%3A")
}
fn unescape(part: &str) -> String {
    part.replace("%3A", ":").replace("%25", "%")
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ItemId {
    Episode { podcast: String, episode: String },
    Show(String),
    Book(String),
}
impl ItemId {
    pub fn encode(&self) -> String {
        match self {
            Self::Episode { podcast, episode } => {
                format!("episode:{}:{}", escape(podcast), escape(episode))
            }
            Self::Show(id) => format!("show:{}", escape(id)),
            Self::Book(id) => format!("book:{}", escape(id)),
        }
    }
    pub fn parse(text: &str) -> Option<Self> {
        let (kind, rest) = text.split_once(':')?;
        match kind {
            "episode" => {
                let (podcast, episode) = rest.split_once(':')?;
                (!podcast.is_empty() && !episode.is_empty()).then(|| Self::Episode {
                    podcast: unescape(podcast),
                    episode: unescape(episode),
                })
            }
            "show" if !rest.is_empty() => Some(Self::Show(unescape(rest))),
            "book" if !rest.is_empty() => Some(Self::Book(unescape(rest))),
            _ => None,
        }
    }
}

/// The current item's queue row.
pub fn now_occurrence(item_id: &str) -> String {
    format!("now:{item_id}")
}
/// An up-next row: Ossicle's queue holds each episode at most once.
pub fn next_occurrence(episode_id: &str) -> String {
    format!("next:{}", escape(episode_id))
}
pub fn parse_next_occurrence(occurrence: &str) -> Option<String> {
    occurrence.strip_prefix("next:").map(unescape)
}

pub fn show_key(podcast_id: &str) -> String {
    format!("show:{}", escape(podcast_id))
}

// ---------- items ----------

/// Year from an RFC 3339 / ISO date such as `2026-08-19T12:00:00Z`.
fn year_of(date: &str) -> Option<u32> {
    let year = date.get(..4)?;
    (date.as_bytes().get(4) == Some(&b'-'))
        .then(|| year.parse().ok())
        .flatten()
}

pub fn progress_of(episode: &Episode) -> Option<Progress> {
    let position = episode.position.as_ref()?;
    Some(Progress {
        played: position.played,
        position_ms: (!position.played && position.seconds > 0)
            .then_some(u64::from(position.seconds) * 1000),
    })
}

pub fn episode_item(episode: &Episode, show_title: Option<&str>) -> Item {
    Item {
        source: SourceId::Ossicle,
        id: ItemId::Episode {
            podcast: episode.podcast_id.clone(),
            episode: episode.id.clone(),
        }
        .encode(),
        kind: ItemKind::Episode,
        title: episode.title.clone(),
        subtitle: show_title.map(str::to_owned),
        album: show_title.map(str::to_owned),
        year: episode.published_at.as_deref().and_then(year_of),
        duration_ms: episode.duration_seconds.map(|s| u64::from(s) * 1000),
        count: None,
        openable: false,
        playable: true,
        progress: progress_of(episode),
    }
}

pub fn show_item(id: &str, title: &str, count: u32) -> Item {
    Item {
        source: SourceId::Ossicle,
        id: ItemId::Show(id.to_owned()).encode(),
        kind: ItemKind::Show,
        title: title.to_owned(),
        subtitle: None,
        album: None,
        year: None,
        duration_ms: None,
        count: Some(u64::from(count)),
        openable: true,
        playable: false,
        progress: None,
    }
}

pub fn book_item(book: &Book) -> Item {
    Item {
        source: SourceId::Ossicle,
        id: ItemId::Book(book.id.clone()).encode(),
        kind: ItemKind::Book,
        title: book.title.clone(),
        subtitle: (!book.authors.is_empty()).then(|| book.authors.join(", ")),
        album: None,
        year: book.year,
        duration_ms: None,
        count: None,
        openable: false,
        playable: true,
        progress: None,
    }
}

/// Case-insensitive match on every word of the query.
pub fn matches(query: &str, fields: &[&str]) -> bool {
    let haystack = fields.join(" ").to_lowercase();
    query
        .to_lowercase()
        .split_whitespace()
        .all(|word| haystack.contains(word))
}

// ---------- settings ----------

#[derive(Clone, Debug, PartialEq)]
pub struct Effective {
    pub speed: f64,
    pub skip: SkipIntervals,
}

/// Account settings with a show's overrides on top (`None` inherits).
pub fn effective(settings: &Settings, show: Option<&PodcastSettings>) -> Effective {
    let clamp_skip = |s: u32| s.clamp(1, 300);
    Effective {
        speed: show
            .and_then(|s| s.playback_speed)
            .unwrap_or(settings.playback_speed)
            .clamp(0.5, 3.0),
        skip: SkipIntervals {
            back_s: clamp_skip(
                show.and_then(|s| s.skip_back_seconds)
                    .unwrap_or(settings.skip_back_seconds),
            ),
            forward_s: clamp_skip(
                show.and_then(|s| s.skip_forward_seconds)
                    .unwrap_or(settings.skip_forward_seconds),
            ),
        },
    }
}

/// Speed in Ossicle's 0.1 steps.
pub fn round_speed(rate: f64) -> f64 {
    ((rate * 10.0).round() / 10.0).clamp(0.5, 3.0)
}

/// Target of a skip by `delta_s`, kept inside the item when its length is known.
pub fn skip_target(position_ms: u64, delta_s: i64, duration_ms: Option<u64>) -> u64 {
    let target = (position_ms as i64 + delta_s * 1000).max(0) as u64;
    match duration_ms {
        Some(duration) => target.min(duration.saturating_sub(1000)),
        None => target,
    }
}

// ---------- sponsor segments ----------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Span {
    pub start_ms: u64,
    pub end_ms: u64,
}

const MERGE_GAP_MS: u64 = 500;
const MIN_REMAINING_MS: u64 = 1000;

/// Ordered spans with touching or overlapping segments merged (Ossicle's player rule).
pub fn merge_segments(segments: &[SponsorSegment]) -> Vec<Span> {
    let mut spans: Vec<Span> = segments
        .iter()
        .filter(|s| s.start_seconds >= 0.0 && s.end_seconds > s.start_seconds)
        .map(|s| Span {
            start_ms: (s.start_seconds * 1000.0).round() as u64,
            end_ms: (s.end_seconds * 1000.0).round() as u64,
        })
        .collect();
    spans.sort_by_key(|s| s.start_ms);
    let mut merged: Vec<Span> = vec![];
    for span in spans {
        match merged.last_mut() {
            Some(last) if span.start_ms <= last.end_ms + MERGE_GAP_MS => {
                last.end_ms = last.end_ms.max(span.end_ms);
            }
            _ => merged.push(span),
        }
    }
    merged
}

pub fn span_at(spans: &[Span], position_ms: u64) -> Option<Span> {
    spans
        .iter()
        .copied()
        .find(|s| position_ms >= s.start_ms && position_ms < s.end_ms)
}

/// Where to jump when playback is inside a sponsor span that the listener
/// did not seek into, unless less than a second of it remains.
pub fn sponsor_jump(spans: &[Span], position_ms: u64, suppressed: &[u64]) -> Option<u64> {
    let span = span_at(spans, position_ms)?;
    (!suppressed.contains(&span.start_ms) && span.end_ms - position_ms >= MIN_REMAINING_MS)
        .then_some(span.end_ms)
}

// ---------- narration ----------

/// The sentence a stored reading position lands in (Ossicle's player rule).
pub fn start_ordinal_for(timeline: &Timeline, locator: Option<&BookLocator>) -> u32 {
    let Some(locator) = locator else { return 0 };
    let in_resource: Vec<&TimelineSegment> = timeline
        .segments
        .iter()
        .filter(|s| s.resource == locator.resource)
        .collect();
    let Some(last) = in_resource.last() else {
        return 0;
    };
    if let Some(segment) = in_resource
        .iter()
        .find(|s| locator.character_offset < s.end_offset)
    {
        return segment.ordinal;
    }
    timeline
        .segments
        .iter()
        .find(|s| s.ordinal > last.ordinal)
        .map_or(last.ordinal, |s| s.ordinal)
}

/// Which sentence of a stitched window sounds at `position_ms`.
pub fn window_index_at(starts_ms: &[u32], position_ms: u64) -> usize {
    starts_ms
        .iter()
        .rposition(|&start| position_ms >= u64::from(start))
        .unwrap_or(0)
}

/// The text anchor that a sentence writes back: listening position is reading position.
pub fn locator_for(
    timeline: &Timeline,
    generation: &str,
    segment: &NarrationSegment,
) -> BookLocator {
    let in_resource: Vec<&TimelineSegment> = timeline
        .segments
        .iter()
        .filter(|s| s.resource == segment.resource)
        .collect();
    let rank = in_resource
        .iter()
        .position(|s| s.ordinal == segment.ordinal);
    let resource_progression = match rank {
        Some(rank) if !in_resource.is_empty() => rank as f32 / in_resource.len() as f32,
        _ => 0.0,
    };
    let whole_book_progression = timeline
        .segments
        .iter()
        .find(|s| s.ordinal == segment.ordinal)
        .filter(|_| timeline.estimated_duration_ms > 0)
        .map_or(0.0, |s| {
            (s.estimated_start_ms as f32 / timeline.estimated_duration_ms as f32).min(1.0)
        });
    BookLocator {
        generation: generation.to_owned(),
        resource: segment.resource.clone(),
        selector: segment.selector.clone(),
        character_offset: segment.start_offset,
        text_context: segment.text.chars().take(200).collect(),
        resource_progression,
        whole_book_progression,
    }
}

/// The timeline's estimate of where a locator sits, for conflict rows.
pub fn estimated_ms(timeline: &Timeline, locator: &BookLocator) -> u64 {
    let ordinal = start_ordinal_for(timeline, Some(locator));
    timeline
        .segments
        .iter()
        .find(|s| s.ordinal == ordinal)
        .map_or(0, |s| u64::from(s.estimated_start_ms))
}

pub fn next_after(timeline: &Timeline, ordinal: u32) -> Option<&TimelineSegment> {
    timeline.segments.iter().find(|s| s.ordinal > ordinal)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sources::ossicle::wire::Position;
    use std::time::Duration;

    fn settings() -> Settings {
        Settings {
            skip_back_seconds: 15,
            skip_forward_seconds: 30,
            playback_speed: 1.2,
            auto_advance_queue: true,
            refresh_minutes: 30,
            sponsor_skip: true,
            continuous_playback: false,
            books_upstream: String::new(),
        }
    }

    #[test]
    fn retry_waits_two_then_ten_seconds_then_gives_up() {
        assert_eq!(retry_delay(1), Some(Duration::from_secs(2)));
        assert_eq!(retry_delay(2), Some(Duration::from_secs(10)));
        assert_eq!(retry_delay(3), None);
    }

    #[test]
    fn ids_round_trip_even_with_separators() {
        for id in [
            ItemId::Episode {
                podcast: "p:1%".into(),
                episode: "e:2".into(),
            },
            ItemId::Show("abc".into()),
            ItemId::Book("b:3".into()),
        ] {
            assert_eq!(ItemId::parse(&id.encode()), Some(id));
        }
        assert_eq!(ItemId::parse("episode:onlyone"), None);
        assert_eq!(ItemId::parse("show:"), None);
        assert_eq!(ItemId::parse("nonsense"), None);
        assert_eq!(
            parse_next_occurrence(&next_occurrence("x:y")).as_deref(),
            Some("x:y")
        );
    }

    #[test]
    fn show_overrides_win_and_values_stay_in_ossicle_ranges() {
        let account = settings();
        assert_eq!(
            effective(&account, None),
            Effective {
                speed: 1.2,
                skip: SkipIntervals {
                    back_s: 15,
                    forward_s: 30
                }
            }
        );
        let show = PodcastSettings {
            podcast_id: "p".into(),
            playback_speed: Some(2.0),
            skip_back_seconds: None,
            skip_forward_seconds: Some(500),
        };
        let e = effective(&account, Some(&show));
        assert_eq!(e.speed, 2.0);
        assert_eq!(e.skip.back_s, 15, "an absent override inherits");
        assert_eq!(e.skip.forward_s, 300);
        assert_eq!(round_speed(1.26), 1.3);
        assert_eq!(round_speed(9.0), 3.0);
    }

    #[test]
    fn skips_stay_inside_the_item() {
        assert_eq!(skip_target(10_000, -15, Some(60_000)), 0);
        assert_eq!(skip_target(10_000, 30, Some(60_000)), 40_000);
        assert_eq!(skip_target(50_000, 30, Some(60_000)), 59_000);
        assert_eq!(skip_target(50_000, 30, None), 80_000);
    }

    #[test]
    fn sponsor_spans_merge_and_skip_unless_suppressed_or_nearly_over() {
        let seg = |a: f64, b: f64| SponsorSegment {
            start_seconds: a,
            end_seconds: b,
        };
        let spans = merge_segments(&[
            seg(10.0, 20.0),
            seg(2.0, 5.0),
            seg(20.3, 25.0),
            seg(7.0, 6.0),
        ]);
        assert_eq!(
            spans,
            vec![
                Span {
                    start_ms: 2000,
                    end_ms: 5000
                },
                Span {
                    start_ms: 10_000,
                    end_ms: 25_000
                }
            ]
        );
        assert_eq!(sponsor_jump(&spans, 2100, &[]), Some(5000));
        assert_eq!(sponsor_jump(&spans, 4500, &[]), None, "under a second left");
        assert_eq!(sponsor_jump(&spans, 12_000, &[10_000]), None);
        assert_eq!(sponsor_jump(&spans, 6000, &[]), None);
    }

    #[test]
    fn items_carry_progress_and_unknowns_stay_absent() {
        let mut episode = Episode {
            id: "e".into(),
            podcast_id: "p".into(),
            title: "T".into(),
            duration_seconds: None,
            published_at: Some("2026-08-19T12:00:00Z".into()),
            audio_path: "/audio/e".into(),
            video_path: None,
            position: None,
        };
        let item = episode_item(&episode, Some("Show"));
        assert_eq!(item.year, Some(2026));
        assert!(item.duration_ms.is_none() && item.progress.is_none());
        episode.position = Some(Position {
            episode_id: "e".into(),
            seconds: 61,
            revision: 3,
            device_id: "phone".into(),
            updated_at: "x".into(),
            played: false,
        });
        assert_eq!(
            progress_of(&episode),
            Some(Progress {
                played: false,
                position_ms: Some(61_000)
            })
        );
        assert!(matches("quiet fog", &["Episode 3: Fog", "The Quiet Hour"]));
        assert!(!matches("rain", &["Episode 3: Fog"]));
    }

    fn timeline() -> Timeline {
        let seg = |ordinal, resource: &str, start, end, ms| TimelineSegment {
            ordinal,
            resource: resource.into(),
            selector: format!("#s{ordinal}"),
            start_offset: start,
            end_offset: end,
            estimated_start_ms: ms,
        };
        Timeline {
            estimated_duration_ms: 10_000,
            segments: vec![
                seg(0, "a", 0, 10, 0),
                seg(1, "a", 10, 20, 2000),
                seg(2, "b", 0, 10, 5000),
            ],
        }
    }

    #[test]
    fn narration_positions_map_both_ways() {
        let t = timeline();
        let locator = |resource: &str, offset| BookLocator {
            generation: "g".into(),
            resource: resource.into(),
            selector: String::new(),
            character_offset: offset,
            text_context: String::new(),
            resource_progression: 0.0,
            whole_book_progression: 0.0,
        };
        assert_eq!(start_ordinal_for(&t, None), 0);
        assert_eq!(start_ordinal_for(&t, Some(&locator("a", 12))), 1);
        assert_eq!(start_ordinal_for(&t, Some(&locator("a", 99))), 2);
        assert_eq!(start_ordinal_for(&t, Some(&locator("zzz", 1))), 0);
        assert_eq!(window_index_at(&[0, 1500, 3000], 1600), 1);
        assert_eq!(window_index_at(&[0, 1500, 3000], 0), 0);
        let segment = NarrationSegment {
            ordinal: 1,
            resource: "a".into(),
            selector: "#s1".into(),
            start_offset: 10,
            text: "Second sentence.".into(),
        };
        let back = locator_for(&t, "g", &segment);
        assert_eq!(back.resource_progression, 0.5);
        assert_eq!(back.whole_book_progression, 0.2);
        assert_eq!(estimated_ms(&t, &back), 2000);
        assert_eq!(next_after(&t, 1).map(|s| s.ordinal), Some(2));
        assert!(next_after(&t, 2).is_none());
    }
}
