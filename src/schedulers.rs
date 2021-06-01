use std::cmp::min;
use std::collections::VecDeque;

use rand::{seq::SliceRandom, Rng};
use rand_seeder::{Seeder, SipRng};

use crate::simulator::{Scheduler, SystemState};
use crate::types::{Batch, Job, Time};
use crate::utils::prelude::*;
use crate::workers::Worker;

/// The simplest FIFO scheduler, with a fixed batch size of 5
#[derive(Debug)]
pub struct FIFO {
    workers: Vec<Worker>,
}

impl FIFO {
    pub fn new(workers: Vec<Worker>) -> Self {
        FIFO { workers }
    }

    fn next_batch(&mut self, now: Time, pending_jobs: &mut VecDeque<Job>) -> Vec<SystemState> {
        let mut new_events = vec![];
        loop {
            if pending_jobs.is_empty() {
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
                    let batch_size = min(w.batch_size(), pending_jobs.len());
                    let batch = pending_jobs.drain(..batch_size).collect();
                    w.batch_start(&batch);
                    new_events.push(SystemState::batch(id, now, batch))
                }
            };
        }
        new_events
    }
}

impl Scheduler for FIFO {
    #[instrument(
        level = "debug",
        skip(self, pending_jobs),
        fields(
            ?now,
            pending_jobs.len = pending_jobs.len()
        )
    )]
    fn on_new_jobs(&mut self, now: Time, pending_jobs: &mut VecDeque<Job>) -> Vec<SystemState> {
        self.next_batch(now, pending_jobs)
    }

    #[instrument(
        level = "debug",
        skip(self, pending_jobs, batch),
        fields(
            ?now,
            pending_jobs.len = pending_jobs.len(),
            batch.id = batch.id,
            ?batch.started,
            batch.jobs.len = batch.jobs.len()
        )
    )]
    fn on_batch_done(&mut self, now: Time, batch: &Batch, pending_jobs: &mut VecDeque<Job>) -> Vec<SystemState> {
        self.workers[batch.id].batch_done(batch);
        self.next_batch(now, pending_jobs)
    }
}

#[derive(Debug)]
pub struct Random<T> {
    workers: Vec<Worker>,
    rng: T,
}

impl<T: Rng> Random<T> {
    pub fn new(rng: T, workers: Vec<Worker>) -> Self {
        Random { workers, rng }
    }

    fn next_batch(&mut self, now: Time, pending_jobs: &mut VecDeque<Job>) -> Vec<SystemState> {
        let mut new_events = vec![];
        loop {
            if pending_jobs.is_empty() {
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
                    let batch_size = min(w.batch_size(), pending_jobs.len());
                    // first make pending_jobs a continuous slice
                    // and shuffle its head
                    pending_jobs
                        .make_contiguous()
                        .partial_shuffle(&mut self.rng, batch_size);
                    let batch = pending_jobs.drain(..batch_size).collect();
                    w.batch_start(&batch);
                    new_events.push(SystemState::batch(id, now, batch))
                }
            };
        }
        new_events
    }
}

impl<T: Rng> Scheduler for Random<T> {
    #[instrument(
        level = "debug",
        skip(self, pending_jobs),
        fields(
            ?now,
            pending_jobs.len = pending_jobs.len()
        )
    )]
    fn on_new_jobs(&mut self, now: Time, pending_jobs: &mut VecDeque<Job>) -> Vec<SystemState> {
        // Random does not preempt
        self.next_batch(now, pending_jobs)
    }

    #[instrument(
        level = "debug",
        skip(self, pending_jobs, batch),
        fields(
            ?now,
            pending_jobs.len = pending_jobs.len(),
            batch.id = batch.id,
            ?batch.started,
            batch.jobs.len = batch.jobs.len()
        )
    )]
    fn on_batch_done(&mut self, now: Time, batch: &Batch, pending_jobs: &mut VecDeque<Job>) -> Vec<SystemState> {
        self.workers[batch.id].batch_done(batch);
        self.next_batch(now, pending_jobs)
    }
}

pub fn from_config(cfg: &SchedulerConfig, rng: impl Rng + 'static, workers: Vec<Worker>) -> Result<Box<dyn Scheduler>> {
    Ok(match cfg {
        SchedulerConfig::FIFO => Box::new(FIFO::new(workers)),
        SchedulerConfig::Random { seed } => {
            if let Some(seed) = seed {
                let rng: SipRng = Seeder::from(seed).make_rng();
                Box::new(Random::new(rng, workers))
            } else {
                Box::new(Random::new(rng, workers))
            }
        }
        SchedulerConfig::Nexus => Box::new(nexus::Nexus),
    })
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
#[serde(tag = "type")]
pub enum SchedulerConfig {
    FIFO,
    Random {
        /// Optional seed. If none, will use the same random generator as the one for incoming jobs
        seed: Option<String>,
    },
    Nexus,
}
