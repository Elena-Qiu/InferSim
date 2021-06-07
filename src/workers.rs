use std::collections::VecDeque;

use itertools::Itertools;

use crate::types::{Batch, Time, TimeInterval};

/// A single timeline tracking pairs of batches
#[derive(Debug, Clone, Default)]
pub struct Timeline {
    /// list of pending batches, each pair is (start, duration), in sorted asc order
    /// The assumption is that the pairs are non-overlapping
    pending: VecDeque<TimeInterval>,
}

impl Timeline {
    fn add(&mut self, pair: TimeInterval) {
        let idx = self
            .pending
            .binary_search_by(|other| other.cmp(&&pair))
            .unwrap_or_else(|idx| idx);
        self.pending.insert(idx, pair);
    }

    fn remove(&mut self, pair: &TimeInterval) {
        if let Ok(idx) = self
            .pending
            .binary_search_by(|other| other.cmp(&pair))
        {
            self.pending.remove(idx);
        }
    }

    /// whether a given time duration is occupied
    /// returns paris that are overlapping with the provided duration
    pub fn occupied(&self, interval: &TimeInterval) -> Vec<TimeInterval> {
        // start with the potential idx of the interval
        match self.pending.binary_search(interval) {
            Ok(idx) => {
                // there's an exact overlap
                vec![self.pending[idx]]
            }
            Err(idx) => {
                // look at previous (idx - 1) and the next (idx, because interval is not really inserted)
                [0usize, 1usize]
                    .iter()
                    // compute idx-1 and idx
                    .filter_map(|offset| idx.checked_sub(*offset))
                    // get pair from the array
                    .filter_map(|idx| self.pending.get(idx))
                    // only retain ones that are overlapping with our probe interval
                    .filter(|pair| pair.overlap(interval))
                    .map(Clone::clone)
                    .collect()
            }
        }
    }

    /// whether the worker has no more jobs since given time
    pub fn idle(&self, since: Time) -> bool {
        // basically an interval extend to the far future
        let probe = TimeInterval(since, f64::MAX.into());
        self.occupied(&probe).is_empty()
    }
}

/// Tracks a worker's state
#[derive(Debug)]
pub struct Worker {
    id: usize,
    batch_size: usize,
    timeline: Timeline,
}

impl Worker {
    pub fn id(&self) -> usize {
        self.id
    }

    pub fn available_slots(&self) -> usize {
        1
    }

    pub fn batch_start(&mut self, batch: &Batch) {
        assert_eq!(self.id, batch.id);
        let interval = batch.to_interval();
        let occupied = self.timeline.occupied(&interval);
        assert!(
            occupied.is_empty(),
            "Scheduler should ensure only start batch on free worker. Tries to start {}, which overlaps with {:?}",
            interval,
            occupied
        );
        self.timeline.add(interval);
    }

    pub fn batch_done(&mut self, batch: &Batch) {
        assert_eq!(self.id, batch.id);
        let interval = batch.to_interval();
        self.timeline.remove(&interval);
    }

    pub fn timeline(&self) -> &Timeline {
        &self.timeline
    }

    pub fn batch_size(&self) -> usize {
        self.batch_size
    }
}

pub fn from_config<'a>(cfg: impl IntoIterator<Item = &'a WorkerConfig>) -> Vec<Worker> {
    cfg.into_iter()
        .enumerate()
        .map(|(id, c)| Worker {
            id,
            batch_size: c.batch_size,
            timeline: Default::default(),
        })
        .collect_vec()
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct WorkerConfig {
    batch_size: usize,
}
