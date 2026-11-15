use crate::settings::Repeat;
use rand::seq::SliceRandom;
use std::collections::VecDeque;

#[derive(Default)]
pub struct Queue {
    pub tracks: Vec<i64>,
    pub current: Option<i64>,
    pub shuffle: bool,
    pub home: bool,
    remaining: VecDeque<i64>,
    history: Vec<i64>,
}

impl Queue {
    pub fn start(
        &mut self,
        tracks: Vec<i64>,
        chosen: Option<i64>,
        shuffle: bool,
        home: bool,
    ) -> Option<i64> {
        self.tracks = tracks;
        self.shuffle = shuffle || home;
        self.home = home;
        self.history.clear();
        self.current = chosen.filter(|id| self.tracks.contains(id));
        self.refill();
        let id = self.current.or_else(|| self.remaining.pop_front());
        self.current = id;
        if let Some(id) = id {
            if self.shuffle {
                self.remaining.retain(|value| *value != id);
            } else {
                let index = self
                    .tracks
                    .iter()
                    .position(|value| *value == id)
                    .unwrap_or(0);
                self.remaining = self.tracks.iter().skip(index + 1).copied().collect();
            }
            self.history.push(id);
        }
        id
    }

    fn refill(&mut self) {
        let mut ids = self.tracks.clone();
        if self.shuffle {
            ids.shuffle(&mut rand::thread_rng());
            if ids.len() > 1 && ids.first().copied() == self.current {
                ids.swap(0, 1);
            }
        }
        self.remaining = ids.into();
    }

    pub fn set_shuffle(&mut self, shuffle: bool) {
        self.shuffle = shuffle || self.home;
        // Keep the already consumed portion of this cycle consumed.
        if self.shuffle {
            self.remaining
                .make_contiguous()
                .shuffle(&mut rand::thread_rng());
        } else if let Some(current) = self.current {
            let index = self
                .tracks
                .iter()
                .position(|id| *id == current)
                .unwrap_or(0);
            self.remaining = self.tracks.iter().skip(index + 1).copied().collect();
        }
    }

    pub fn advance(&mut self, repeat: Repeat, manual: bool) -> Option<i64> {
        if !manual && repeat == Repeat::One {
            return self.current;
        }
        if self.remaining.is_empty() {
            if repeat == Repeat::Off && !self.home && !manual {
                return None;
            }
            self.refill();
        }
        let id = self.remaining.pop_front()?;
        self.current = Some(id);
        self.history.push(id);
        if self.history.len() > 256 {
            self.history.remove(0);
        }
        Some(id)
    }

    pub fn previous(&mut self) -> Option<i64> {
        if self.history.len() > 1 {
            if let Some(leaving) = self.history.pop() {
                self.remaining.push_front(leaving);
            }
            self.current = self.history.last().copied();
        }
        self.current
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shuffle_visits_every_track_and_avoids_cycle_boundary_repeat() {
        let mut queue = Queue::default();
        let first = queue.start(vec![1, 2, 3, 4, 5], None, true, true).unwrap();
        let mut cycle = vec![first];
        for _ in 0..4 {
            cycle.push(queue.advance(Repeat::All, false).unwrap());
        }
        let last = *cycle.last().unwrap();
        cycle.sort();
        assert_eq!(cycle, vec![1, 2, 3, 4, 5]);
        assert_ne!(queue.advance(Repeat::All, false), Some(last));
    }
    #[test]
    fn home_always_shuffles_and_continues_with_repeat_off() {
        let mut queue = Queue::default();
        queue.start(vec![1, 2], Some(1), false, true);
        queue.set_shuffle(false);
        assert!(queue.shuffle);
        assert_eq!(queue.advance(Repeat::Off, false), Some(2));
        assert_eq!(queue.advance(Repeat::Off, false), Some(1));
    }
    #[test]
    fn sequential_repeat_and_manual_next_have_distinct_behavior() {
        let mut queue = Queue::default();
        assert_eq!(queue.start(vec![1, 2], None, false, false), Some(1));
        assert_eq!(queue.advance(Repeat::One, false), Some(1));
        assert_eq!(queue.advance(Repeat::One, true), Some(2));
        assert_eq!(queue.advance(Repeat::Off, false), None);
        assert_eq!(queue.previous(), Some(1));
    }
    #[test]
    fn selecting_a_track_continues_from_its_album_position() {
        let mut queue = Queue::default();
        assert_eq!(
            queue.start(vec![1, 2, 3, 4], Some(3), false, false),
            Some(3)
        );
        assert_eq!(queue.advance(Repeat::Off, false), Some(4));
        assert_eq!(queue.advance(Repeat::Off, false), None);
    }
}
