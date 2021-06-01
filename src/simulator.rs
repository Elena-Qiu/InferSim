use std::collections::{BinaryHeap, VecDeque};
use std::fmt;

use crate::incoming::Incoming;
use crate::types::{Batch, Duration, IncomingJob, Job, Time};
use crate::utils::logging::prelude::*;
use crate::EndCondition;
use std::cmp::{Ordering, Reverse};

/// The simulation state
#[derive(Debug, Clone)]
pub enum SystemState {
    /// Run this batch of jobs
    BatchDone(Batch),
    /// Startup
    Start,
    /// do nothing and wait for other events
    Idle,
    /// State/event injected by scheduler_process for all incoming jobs
    IncomingJobs { jobs: Vec<IncomingJob> },
    /// Some jobs are past due
    JobsPastDue(Vec<Job>),
}

impl SystemState {
    pub fn batch(id: usize, now: Time, jobs: Vec<Job>) -> Self {
        Self::BatchDone(Batch { id, jobs, started: now })
    }
}

impl fmt::Display for SystemState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SystemState::BatchDone(batch) => write!(f, "BatchDone({})", batch),
            SystemState::Start => write!(f, "Start"),
            SystemState::Idle => write!(f, "Idle"),
            SystemState::IncomingJobs { jobs, .. } => write!(f, "IncomingJobs {{ jobs.len: {} }}", jobs.len()),
            SystemState::JobsPastDue(jobs) => write!(f, "JobsPastDue( len: {} )", jobs.len()),
        }
    }
}

impl Default for SystemState {
    fn default() -> Self {
        Self::Idle
    }
}

enum Effect {
    Timeout(Duration),
    Wait,
}

impl SystemState {
    fn get_effect(&self) -> Effect {
        match self {
            SystemState::IncomingJobs { .. } => Effect::Timeout(Duration(0.)),
            SystemState::JobsPastDue(_) => {
                warn!(?self, "Some jobs are past due");
                Effect::Timeout(Duration(0.))
            }
            SystemState::BatchDone(batch) => Effect::Timeout(batch.latency()),
            SystemState::Start => Effect::Timeout(Duration(0.)),
            SystemState::Idle => Effect::Wait,
        }
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

#[derive(Debug, Clone)]
pub struct Event {
    pub time: Time,
    pub state: SystemState,
}

impl fmt::Display for Event {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "@{:.2} -> {}", self.time.0, self.state)
    }
}

impl PartialEq for Event {
    fn eq(&self, other: &Self) -> bool {
        self.time.0 == other.time.0
    }
}
impl Eq for Event {}

impl PartialOrd for Event {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.time.partial_cmp(&other.time)
    }
}
impl Ord for Event {
    fn cmp(&self, other: &Self) -> Ordering {
        self.time.0.total_cmp(&other.time.0)
    }
}

pub(crate) fn schedule_loop(mut scheduler: impl Scheduler, incoming_jobs: Incoming, until: EndCondition) -> Vec<Event> {
    // simulation state
    let mut future_events = BinaryHeap::<Reverse<Event>>::new();
    future_events.push(Reverse(Event {
        time: 0.0.into(),
        state: SystemState::Start,
    }));
    let mut time: Time;
    let mut processed_events: Vec<Event> = Default::default();

    // scheduler state
    let mut incoming_jobs = incoming_jobs.into_absolute();
    let mut pending_jobs: VecDeque<Job> = Default::default();

    while let Some(Reverse(event)) = future_events.pop() {
        let _s = debug_span!("event", event.time = %event.time, event.state = %event.state).entered();
        time = event.time;
        // handle past due jobs
        pending_jobs = {
            let (past_due, pending_jobs): (VecDeque<_>, _) = pending_jobs
                .into_iter()
                .partition(|j| j.missed_deadline(time));
            if !past_due.is_empty() {
                let new_event = Event {
                    time,
                    state: SystemState::JobsPastDue(past_due.into()),
                };
                info!(%new_event, "push event");
                future_events.push(Reverse(new_event));
            }
            pending_jobs
        };
        // record the event now because the scheduler will consume the event
        processed_events.push(event.clone());
        // invoke the scheduler
        {
            scheduler.on_tick(time);
            let next = match event.state {
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
                _ => SystemState::Idle,
            };
            match next.get_effect() {
                Effect::Timeout(d) => {
                    let new_event = Event {
                        time: time + d,
                        state: next,
                    };
                    info!(%new_event, "push event");
                    future_events.push(Reverse(new_event));
                }
                Effect::Wait => {}
            }
        };
        // handle incoming
        // poll the incoming generator if it's not done until
        // the next incoming job is after the latest batch done
        while future_events
            .peek()
            .map(|e| matches!(e.0.state, SystemState::BatchDone(..)))
            // if no future event, try polling anyways
            .unwrap_or(true)
        {
            debug!(%time, "polling new incoming jobs");
            if let Some((new_time, batch)) = incoming_jobs.next() {
                let new_event = Event {
                    time: new_time,
                    state: SystemState::IncomingJobs { jobs: batch },
                };
                info!(%new_event, "push event");
                future_events.push(Reverse(new_event));
            } else {
                break;
            }
        }
        // end condition
        match until {
            EndCondition::Time(t) => {
                if time > t {
                    break;
                }
            }
            EndCondition::NoEvents => {}
        }
    }

    processed_events
}
