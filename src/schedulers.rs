use std::cmp::min;
use std::collections::VecDeque;

use crate::simulator::{Scheduler, SchedulerState};
use crate::utils::logging::prelude::*;
use crate::{Batch, Job};

/// The simplest FIFO scheduler, with a fixed batch size of 5
#[derive(Debug)]
pub struct FIFO {
    batch_size: usize,
    running: bool,
}

impl FIFO {
    pub fn new(batch_size: usize) -> Self {
        FIFO {
            batch_size,
            running: false,
        }
    }

    fn next_batch(&mut self, pending_jobs: &mut VecDeque<Job>) -> SchedulerState {
        let batch_size = min(self.batch_size, pending_jobs.len());
        if batch_size == 0 {
            return SchedulerState::wait();
        }

        assert!(!self.running);
        self.running = true;
        let batch = pending_jobs.drain(..batch_size).collect();
        SchedulerState::batch(batch)
    }
}

impl Scheduler for FIFO {
    #[instrument(
        level = "debug",
        skip(self, pending_jobs),
        fields(
            %self.running,
            pending_jobs.len = pending_jobs.len()
        )
    )]
    fn on_new_jobs(&mut self, pending_jobs: &mut VecDeque<Job>) -> SchedulerState {
        // FIFO does not preempt
        if self.running {
            SchedulerState::wait()
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
    fn on_batch_done(&mut self, _: &Batch, pending_jobs: &mut VecDeque<Job>) -> SchedulerState {
        self.running = false;
        self.next_batch(pending_jobs)
    }
}
