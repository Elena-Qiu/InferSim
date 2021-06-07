use std::cmp::min;
use std::collections::VecDeque;

use nuts::{ActivityId, DefaultDomain, DomainState};
use rand::Rng;
use rand_seeder::Seeder;

use crate::types::{Job, Time};
use crate::utils::prelude::*;
use crate::utils::Batcher;
use crate::workers::Worker;

use super::msg;
use super::Simulation;
use rand_seeder::SipRng;

mod from_config;
pub use from_config::{from_config, SchedulerConfig};

pub trait Scheduler {
    fn on_incoming_jobs(&mut self, state: &mut Simulation, msg: &msg::IncomingJobs);

    fn on_batch_done(&mut self, state: &mut Simulation, msg: &msg::BatchDone);
}

impl Scheduler for Box<dyn Scheduler> {
    #[inline]
    fn on_incoming_jobs(&mut self, state: &mut Simulation, msg: &msg::IncomingJobs) {
        (**self).on_incoming_jobs(state, msg)
    }

    #[inline]
    fn on_batch_done(&mut self, state: &mut Simulation, msg: &msg::BatchDone) {
        (**self).on_batch_done(state, msg)
    }
}

/// put pending jobs onto all available workers
fn drain_available_worker(now: Time, workers: &[Worker], mut get_batch: impl FnMut(usize) -> Vec<Job>) {
    for w in workers {
        if w.timeline().idle(now) {
            let jobs = get_batch(w.batch_size());
            if jobs.is_empty() {
                break;
            }
            nuts::publish(msg::BatchStart {
                when: now,
                which: w.id(),
                jobs,
            });
        }
    }
}

#[derive(Debug)]
pub struct FIFO;

impl FIFO {
    fn next_batch(&self, state: &mut Simulation) {
        let pending = &mut state.pending_jobs;
        drain_available_worker(state.time, &state.workers, |s| pending.batch_pop_front(s));
    }
}

impl Scheduler for FIFO {
    fn on_incoming_jobs(&mut self, state: &mut Simulation, _: &msg::IncomingJobs) {
        self.next_batch(state)
    }

    fn on_batch_done(&mut self, state: &mut Simulation, _: &msg::BatchDone) {
        self.next_batch(state)
    }
}

#[derive(Debug)]
pub struct Random<T> {
    rng: T,
}

impl<T: Rng> Random<T> {
    fn next_batch(&mut self, state: &mut Simulation) {
        let pending = &mut state.pending_jobs;
        drain_available_worker(state.time, &state.workers, |s| {
            pending.batch_pop_front_random(&mut self.rng, s)
        });
    }
}

impl<T: Rng> Scheduler for Random<T> {
    fn on_incoming_jobs(&mut self, state: &mut Simulation, _: &msg::IncomingJobs) {
        self.next_batch(state)
    }

    fn on_batch_done(&mut self, state: &mut Simulation, _: &msg::BatchDone) {
        self.next_batch(state)
    }
}

#[derive(Debug)]
pub struct My {
    percentile: f64,
}

impl My {
    fn next_batch(&mut self, state: &mut Simulation) {
        // pick jobs with deadline first
        let (mut with_deadline, mut best_effort): (Vec<_>, _) = state
            .pending_jobs
            .drain(..)
            .partition(|j| j.deadline.is_some());
        // assume we always start deadline job until have to, with `percentile` possibility not missing the deadline
        // sort deadline jobs by this start time
        with_deadline.sort_by_cached_key(|a| a.deadline.unwrap() - a.length.quantile(self.percentile));
        // now take them until the workers are all currently busy
        drain_available_worker(state.time, &state.workers, |s| with_deadline.batch_pop_front(s));
        // take be jobs if there's still spot
        drain_available_worker(state.time, &state.workers, |s| best_effort.batch_pop_front(s));
        // put them back to pending if anything left
        state
            .pending_jobs
            .extend(with_deadline.into_iter());
        state.pending_jobs.extend(best_effort.into_iter());
    }
}

impl Scheduler for My {
    fn on_incoming_jobs(&mut self, state: &mut Simulation, _: &msg::IncomingJobs) {
        self.next_batch(state)
    }

    fn on_batch_done(&mut self, state: &mut Simulation, _: &msg::BatchDone) {
        self.next_batch(state)
    }
}
