use crate::simulator::{Scheduler, SystemState};
use crate::types::{Batch, Job, Time};
use crate::workers::Worker;
use std::cmp::min;
use std::collections::VecDeque;

#[derive(Debug)]
pub struct My {
    /// which percentile to look at
    percentile: f64,
    workers: Vec<Worker>,
}

impl My {
    pub fn new(workers: Vec<Worker>, percentile: f64) -> Self {
        // only support one worker at the time
        assert_eq!(workers.len(), 1);

        Self { percentile, workers }
    }

    fn next_batch(&mut self, now: Time, pending_jobs: &mut VecDeque<Job>) -> Vec<SystemState> {
        // pick jobs with deadline first
        let (mut with_deadline, mut best_effort): (Vec<_>, _) = pending_jobs
            .drain(..)
            .partition(|j| j.deadline.is_some());
        // sort deadline jobs with latest start time
        with_deadline.sort_by_key(|a| a.deadline.unwrap() - a.length.quantile(self.percentile));
        // take them one by one
        let mut new_events = vec![];
        loop {
            if with_deadline.is_empty() {
                break;
            }
            match self
                .workers
                .iter_mut()
                .enumerate()
                .find(|(_, w)| w.available_slots() > 0)
            {
                None => break,
                Some((id, w)) => {
                    let batch_size = min(w.batch_size(), with_deadline.len());
                    let batch = with_deadline.drain(..batch_size).collect();
                    w.batch_start(&batch);
                    new_events.push(SystemState::batch(id, now, batch))
                }
            };
        }
        // take be jobs
        loop {
            if best_effort.is_empty() {
                break;
            }
            match self
                .workers
                .iter_mut()
                .enumerate()
                .find(|(_, w)| w.available_slots() > 0)
            {
                None => break,
                Some((id, w)) => {
                    let batch_size = min(w.batch_size(), best_effort.len());
                    let batch = best_effort.drain(..batch_size).collect();
                    w.batch_start(&batch);
                    new_events.push(SystemState::batch(id, now, batch))
                }
            };
        }
        // put back pending
        pending_jobs.extend(with_deadline.into_iter());
        pending_jobs.extend(best_effort.into_iter());
        new_events
    }
}

impl Scheduler for My {
    fn on_new_jobs(&mut self, now: Time, pending_jobs: &mut VecDeque<Job>) -> Vec<SystemState> {
        self.next_batch(now, pending_jobs)
    }

    fn on_batch_done(&mut self, now: Time, batch: &Batch, pending_jobs: &mut VecDeque<Job>) -> Vec<SystemState> {
        self.workers[batch.id].batch_done(batch);
        self.next_batch(now, pending_jobs)
    }
}
