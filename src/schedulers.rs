use std::cmp::min;
use std::collections::VecDeque;

use crate::simulator::{Scheduler, SystemState};
use crate::utils::logging::prelude::*;
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
        SystemState::batch(self.now, batch)
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
