use std::cmp::min;
use std::collections::VecDeque;

use rand::{seq::SliceRandom, Rng};

use crate::simulator::{Scheduler, SystemState};
use crate::utils::prelude::*;
use crate::{Batch, Job};

/// The simplest FIFO scheduler, with a fixed batch size of 5
#[derive(Debug)]
pub struct FIFO {
    batch_size: usize,
    running: bool,
    now: f64,
}

impl FIFO {
    pub fn new(batch_size: usize) -> Self {
        FIFO {
            batch_size,
            running: false,
            now: 0.0,
        }
    }

    fn next_batch(&mut self, pending_jobs: &mut VecDeque<Job>) -> SystemState {
        let batch_size = min(self.batch_size, pending_jobs.len());
        if batch_size == 0 {
            return SystemState::wait();
        }

        assert!(!self.running);
        self.running = true;
        let batch = pending_jobs.drain(..batch_size).collect();
        SystemState::batch(0, self.now, batch)
    }
}

impl Scheduler for FIFO {
    fn on_tick(&mut self, now: f64) {
        self.now = now;
    }

    #[instrument(
        level = "debug",
        skip(self, pending_jobs),
        fields(
            %self.running,
            pending_jobs.len = pending_jobs.len()
        )
    )]
    fn on_new_jobs(&mut self, pending_jobs: &mut VecDeque<Job>) -> SystemState {
        // FIFO does not preempt
        if self.running {
            SystemState::wait()
        } else {
            self.next_batch(pending_jobs)
        }
    }

    #[instrument(
        level = "debug",
        skip(self, pending_jobs),
        fields(
            %self.running,
            pending_jobs.len = pending_jobs.len()
        )
    )]
    fn on_batch_done(&mut self, _: &Batch, pending_jobs: &mut VecDeque<Job>) -> SystemState {
        self.running = false;
        self.next_batch(pending_jobs)
    }
}

#[derive(Debug)]
pub struct Random<T> {
    batch_size: usize,
    running: bool,
    now: f64,
    rng: T,
}

impl<T: Rng> Random<T> {
    pub fn new(rng: T, batch_size: usize) -> Self {
        Random {
            batch_size,
            running: false,
            now: 0.0,
            rng,
        }
    }

    fn next_batch(&mut self, pending_jobs: &mut VecDeque<Job>) -> SystemState {
        let batch_size = min(self.batch_size, pending_jobs.len());
        if batch_size == 0 {
            return SystemState::wait();
        }

        assert!(!self.running);
        self.running = true;

        // first make pending_jobs a continuous slice
        // and shuffle its head
        pending_jobs
            .make_contiguous()
            .partial_shuffle(&mut self.rng, batch_size);
        let batch = pending_jobs.drain(..batch_size).collect();
        SystemState::batch(0, self.now, batch)
    }
}

impl<T: Rng> Scheduler for Random<T> {
    fn on_tick(&mut self, now: f64) {
        self.now = now;
    }

    #[instrument(
        level = "debug",
        skip(self, pending_jobs),
        fields(
            %self.running,
            pending_jobs.len = pending_jobs.len()
        )
    )]
    fn on_new_jobs(&mut self, pending_jobs: &mut VecDeque<Job>) -> SystemState {
        // Random does not preempt
        if self.running {
            SystemState::wait()
        } else {
            self.next_batch(pending_jobs)
        }
    }

    #[instrument(
        level = "debug",
        skip(self, pending_jobs),
        fields(
            %self.running,
            pending_jobs.len = pending_jobs.len()
        )
    )]
    fn on_batch_done(&mut self, _: &Batch, pending_jobs: &mut VecDeque<Job>) -> SystemState {
        self.running = false;
        self.next_batch(pending_jobs)
    }
}

pub fn from_config(rng: impl Rng + 'static, cfg: &SchedulerConfig) -> Result<Box<dyn Scheduler>> {
    Ok(match cfg {
        SchedulerConfig::FIFO { batch_size } => Box::new(FIFO::new(*batch_size)),
        SchedulerConfig::Random { batch_size } => Box::new(Random::new(rng, *batch_size)),
    })
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
#[serde(tag = "type")]
pub enum SchedulerConfig {
    FIFO { batch_size: usize },
    Random { batch_size: usize },
}
