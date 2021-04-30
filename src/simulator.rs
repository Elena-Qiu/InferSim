use std::collections::VecDeque;
use std::fmt;

use desim::{Effect, SimContext, SimGen, SimState, Simulation};

use crate::utils::logging::prelude::*;
use crate::{Batch, Job};

#[derive(Debug, Clone)]
pub enum SchedulerState {
    /// Run this batch of jobs
    BatchDone(Batch),
    /// Wait until something happens
    Idle,
    /// State/event injected by the runner for all incoming jobs
    /// should not be returned by the scheduler itself
    IncomingJobs { jobs: Vec<Job> },
}

impl fmt::Display for SchedulerState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BatchDone(batch) => write!(f, "BatchDone({})", batch),
            SchedulerState::Idle => write!(f, "Idle"),
            SchedulerState::IncomingJobs { jobs } => write!(f, "IncomingJobs {{ jobs.len: {} }}", jobs.len()),
        }
    }
}

impl Default for SchedulerState {
    fn default() -> Self {
        Self::Idle
    }
}

impl SchedulerState {
    pub fn incoming_jobs(jobs: Vec<Job>) -> Self {
        Self::IncomingJobs { jobs }
    }
    pub fn batch(jobs: Vec<Job>) -> Self {
        Self::BatchDone(Batch { jobs })
    }
    pub fn wait() -> Self {
        Self::Idle
    }
}

// Implement SimState for reference, so both SchedulerEvent itself and SimStateType can get the impl
impl SimState for SchedulerState {
    fn get_effect(&self) -> Effect {
        match self {
            SchedulerState::IncomingJobs { .. } => {
                error!(?self, "Scheduler should not return IncomingJobs");
                Effect::Trace
            }
            SchedulerState::BatchDone(batch) => Effect::TimeOut(batch.latency()),
            SchedulerState::Idle => Effect::Wait,
        }
    }

    fn set_effect(&mut self, _: Effect) {
        todo!()
    }

    fn should_log(&self) -> bool {
        true
    }
}

/// Scheduler mostly just react on events
pub trait Scheduler {
    fn on_new_jobs(&mut self, pending_jobs: &mut VecDeque<Job>) -> SchedulerState;
    fn on_batch_done(&mut self, batch: &Batch, pending_jobs: &mut VecDeque<Job>) -> SchedulerState;
}

/// The scheduler process will take incoming jobs and create batches from them
fn schedule_process(mut scheduler: impl Scheduler + 'static) -> Box<SimGen<SchedulerState>> {
    Box::new(move |mut ctx: SimContext<SchedulerState>| {
        let mut pending_jobs = VecDeque::new();
        loop {
            let next = {
                let time = ctx.time();
                let curr = ctx.into_state();
                let s = debug_span!("scheduler_iter", %time, %curr);
                let _g = s.enter();

                let next = match curr {
                    SchedulerState::IncomingJobs { jobs, .. } => {
                        // new jobs coming in before current batch finish
                        pending_jobs.extend(jobs.into_iter());
                        scheduler.on_new_jobs(&mut pending_jobs)
                    }
                    SchedulerState::BatchDone(batch) => {
                        // current batch finished
                        scheduler.on_batch_done(&batch, &mut pending_jobs)
                    }
                    SchedulerState::Idle => {
                        // the scheduler doesn't want to do anything
                        // or there's nothing to do
                        // this essentially pass the control back to incoming_jobs_process
                        SchedulerState::wait()
                    }
                };
                info!(%next, "generated next state");
                next
            };
            ctx = yield next;
        }
    })
}

pub fn build_simulation<S, IJ, J>(scheduler: S, incoming_jobs: IJ) -> Simulation<SchedulerState>
where
    S: Scheduler + 'static,
    IJ: IntoIterator<Item = (f64, J)>,
    J: IntoIterator<Item = Job>,
{
    let mut sim = Simulation::new();
    let p_schedule = sim.create_process(schedule_process(scheduler));

    // pump in incoming jobs as events ahead of time
    for (time, batch) in incoming_jobs.into_iter() {
        let jobs = batch.into_iter().collect();
        sim.schedule_event(time, p_schedule, SchedulerState::incoming_jobs(jobs))
    }

    sim
}
