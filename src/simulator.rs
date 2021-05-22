use std::collections::VecDeque;
use std::fmt;

use desim::{Effect, SimContext, SimGen, SimState, Simulation};

use crate::types::{Batch, IncomingJob, Job, Time};
use crate::utils::logging::prelude::*;

/// The simulation state
#[derive(Debug, Clone)]
pub enum SystemState {
    /// Run this batch of jobs
    BatchDone(Batch),
    /// Wait until something happens
    Idle,
    /// State/event injected by the runner for all incoming jobs
    /// should not be returned by the scheduler itself
    IncomingJobs { jobs: Vec<IncomingJob> },
    /// Some jobs are past due
    JobsPastDue(Vec<Job>),
}

impl SystemState {
    pub fn incoming_jobs(jobs: Vec<IncomingJob>) -> Self {
        Self::IncomingJobs { jobs }
    }
    pub fn batch(id: usize, now: Time, jobs: Vec<Job>) -> Self {
        Self::BatchDone(Batch { id, jobs, started: now })
    }
    pub fn wait() -> Self {
        Self::Idle
    }
}

impl fmt::Display for SystemState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SystemState::BatchDone(batch) => write!(f, "BatchDone({})", batch),
            SystemState::Idle => write!(f, "Idle"),
            SystemState::IncomingJobs { jobs } => write!(f, "IncomingJobs {{ jobs.len: {} }}", jobs.len()),
            SystemState::JobsPastDue(jobs) => write!(f, "JobsPastDue( len: {} )", jobs.len()),
        }
    }
}

impl Default for SystemState {
    fn default() -> Self {
        Self::Idle
    }
}

// Implement SimState for reference, so both SchedulerEvent itself and SimStateType can get the impl
impl SimState for SystemState {
    fn get_effect(&self) -> Effect {
        match self {
            SystemState::IncomingJobs { .. } => {
                error!(?self, "Scheduler should not return IncomingJobs");
                Effect::Trace
            }
            SystemState::JobsPastDue(_) => {
                warn!(?self, "Some jobs are past due");
                Effect::Trace
            }
            SystemState::BatchDone(batch) => Effect::TimeOut(batch.latency().0),
            SystemState::Idle => Effect::Wait,
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
    fn on_tick(&mut self, now: Time);
    fn on_new_jobs(&mut self, pending_jobs: &mut VecDeque<Job>) -> SystemState;
    fn on_batch_done(&mut self, batch: &Batch, pending_jobs: &mut VecDeque<Job>) -> SystemState;
}

impl Scheduler for Box<dyn Scheduler> {
    #[inline]
    fn on_tick(&mut self, now: Time) {
        (**self).on_tick(now)
    }

    #[inline]
    fn on_new_jobs(&mut self, pending_jobs: &mut VecDeque<Job>) -> SystemState {
        (**self).on_new_jobs(pending_jobs)
    }

    #[inline]
    fn on_batch_done(&mut self, batch: &Batch, pending_jobs: &mut VecDeque<Job>) -> SystemState {
        (**self).on_batch_done(batch, pending_jobs)
    }
}

/// The scheduler process will take incoming jobs and create batches from them
fn schedule_process(mut scheduler: impl Scheduler + 'static) -> Box<SimGen<SystemState>> {
    Box::new(move |mut ctx: SimContext<SystemState>| {
        let mut pending_jobs: VecDeque<Job> = Default::default();
        loop {
            let time = Time(ctx.time());
            let curr = ctx.into_state();
            // handle past due jobs
            pending_jobs = {
                let (past_due, pending_jobs): (VecDeque<_>, _) = pending_jobs
                    .into_iter()
                    .partition(|j| j.missed_deadline(time));
                if !past_due.is_empty() {
                    ctx = yield SystemState::JobsPastDue(past_due.into());
                    // the time should be the same
                    assert!((time - Time(ctx.time())).abs() < f64::EPSILON);
                }
                pending_jobs
            };
            scheduler.on_tick(time);
            let next = {
                let _s = debug_span!("scheduler_iter", %time, %curr).entered();

                let next = match curr {
                    SystemState::IncomingJobs { jobs, .. } => {
                        // new jobs coming in, accept as pending jobs
                        pending_jobs.extend(jobs.into_iter().map(|ij| ij.into_job(time)));
                        scheduler.on_new_jobs(&mut pending_jobs)
                    }
                    SystemState::BatchDone(batch) => {
                        // current batch finished
                        scheduler.on_batch_done(&batch, &mut pending_jobs)
                    }
                    // other states are irrelevant
                    _ => SystemState::wait(),
                };
                info!(%next, "generated next state");
                next
            };
            ctx = yield next;
        }
    })
}

pub fn build_simulation<S, IJ, J>(scheduler: S, incoming_jobs: IJ) -> Simulation<SystemState>
where
    S: Scheduler + 'static,
    IJ: IntoIterator<Item = (f64, J)>,
    J: IntoIterator<Item = IncomingJob>,
{
    let mut sim = Simulation::new();
    let p_schedule = sim.create_process(schedule_process(scheduler));

    // pump in incoming jobs as events ahead of time
    for (time, batch) in incoming_jobs.into_iter() {
        let jobs = batch.into_iter().collect();
        sim.schedule_event(time, p_schedule, SystemState::incoming_jobs(jobs))
    }

    sim
}
