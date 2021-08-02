// TODO: remove this after the module is hooked up with the outside world
#![allow(unused, dead_code)]

use std::cell::RefCell;
use std::cmp::Reverse;
use std::collections::{BinaryHeap, VecDeque};
use std::default::Default;
use std::iter;
use std::rc::Rc;

use educe::Educe;
use nuts::{ActivityId, DefaultDomain, DomainState};
use parse_display::Display;

use crate::incoming::{Incoming, IncomingAbsolute};
use crate::sim::schedulers::Scheduler;
use crate::types::{Batch, IncomingJob, Job, Time, TimeInterval};
use crate::utils::prelude::*;
use crate::workers::Worker;
use crate::EndCondition;
use std::ops::DerefMut;

pub mod msg;
pub(crate) mod schedulers;

/// Internal event to drive the SimulationController
struct RunStep;
/// Internal event for WorkerController to signal a batch is done
struct InternalBatchDone;

/// Events processed by various controllers
#[derive(Debug, Clone, Ord, PartialOrd, Eq, PartialEq, Display)]
#[display("@{time:.2} -> {message}")]
pub struct Event {
    pub time: Time,
    pub message: Message,
}

macro_rules! define_message {
    ( $( $msg:ident ),+ ) => {

        #[derive(Debug, Clone, Display, Educe)]
        #[educe(PartialEq, Eq, PartialOrd, Ord)]
        #[display("{}")]
        pub enum Message {
            $(
                $msg(
                    #[educe(PartialEq(ignore))]
                    #[educe(PartialOrd(ignore))]
                    #[educe(Ord(ignore))]
                    msg::$msg
                )
            ),+
        }

        $(
            impl From<msg::$msg> for Message {
                fn from(v: msg::$msg) -> Self {
                    Self::$msg(v)
                }
            }
        )+
    };
}

define_message![
    WakeUpSchedulerController,
    WakeUpWorkerController,
    WakeUpIncomingController,
    IncomingJobs,
    PastDue,
    BatchStart,
    BatchDone
];

pub struct Simulation {
    // states related to the whole simulation
    processed_events: Vec<Event>,
    future_events: BinaryHeap<Reverse<Event>>,
    time: Time,

    // states related to the cluster
    pending_jobs: VecDeque<Job>,
    workers: Vec<Worker>,

    // the activity registry
    backend: ActivityId<SimulationController>,
    incoming_controller: ActivityId<IncomingController>,
    worker_controller: ActivityId<WorkerController>,
    scheduler_controller: ActivityId<SchedulerController>,
}

type SimulationState = Rc<RefCell<Simulation>>;

/// The internal controller that actually drives the simulation
struct SimulationController;

impl SimulationController {
    pub fn new() -> ActivityId<Self> {
        let this = nuts::new_domained_activity(Self, &DefaultDomain);
        this.private_domained_channel(Self::step);
        this.private_domained_channel(Self::new_event);

        this
    }

    /// one iter of the simulation loop
    fn step(&mut self, state: &mut DomainState, _: RunStep) {
        let state: &SimulationState = state.get();
        let mut state = state.borrow_mut();

        if let Some(Reverse(event)) = state.future_events.pop() {
            state.time = event.time;
            info!(time = %state.time, %event, "handling event");
            // record the event now because publishing will consume the event
            state.processed_events.push(event.clone());
            // dispatch events to other components
            match event.message {
                Message::WakeUpSchedulerController(inner) => nuts::publish(inner),
                Message::WakeUpWorkerController(inner) => nuts::publish(inner),
                Message::WakeUpIncomingController(inner) => nuts::publish(inner),
                Message::IncomingJobs(inner) => nuts::publish(inner),
                Message::PastDue(inner) => nuts::publish(inner),
                Message::BatchStart(inner) => nuts::publish(inner),
                Message::BatchDone(inner) => nuts::publish(inner),
            }
        }
    }

    /// push the new event into queue
    fn new_event(&mut self, state: &mut DomainState, event: Event) {
        let state: &SimulationState = state.get();
        let mut state = state.borrow_mut();
        info!(time = %state.time, %event, "push event");
        state.future_events.push(Reverse(event));
    }
}

/// A little helper trait to post event on SimulationController
trait PostEvent {
    fn post_event(&self, time: impl Into<Time>, msg: impl Into<Message>);
}

impl PostEvent for ActivityId<SimulationController> {
    fn post_event(&self, time: impl Into<Time>, msg: impl Into<Message>) {
        self.private_message(Event {
            time: time.into(),
            message: msg.into(),
        });
    }
}

/// Drives the incoming iterator, and injects events
struct IncomingController {
    incoming: iter::Peekable<IncomingAbsolute<'static>>,
}

impl IncomingController {
    pub fn new(incoming: Incoming<'static>) -> ActivityId<Self> {
        let this = nuts::new_domained_activity(
            Self {
                incoming: incoming.into_absolute().peekable(),
            },
            &DefaultDomain,
        );
        this.subscribe_domained(Self::run);

        this
    }

    /// This is called when ever there's a wake up event, and is expected to re-schedule itself according to
    /// the incoming job plan
    fn run(&mut self, state: &mut DomainState, _: &msg::WakeUpIncomingController) {
        let state: &SimulationState = state.get();
        let state = state.borrow();

        // post an event if simulation time goes beyond its release time
        if self
            .incoming
            .peek()
            .map(|(t, _)| state.time >= *t)
            .unwrap_or(false)
        {
            let (time, jobs) = self.incoming.next().unwrap();
            info!(time = %state.time, release = %time, jobs.len = jobs.len(), "incoming jobs");
            state
                .backend
                .post_event(time, msg::IncomingJobs { jobs });
        }
        // schedule a wake up if there's more to come
        if let Some(next) = self.incoming.peek().map(|(t, _)| *t) {
            state
                .backend
                .post_event(next, msg::WakeUpIncomingController);
        }
    }
}

/// Tracks execution plan, generate BatchDone events
struct WorkerController;

impl WorkerController {
    pub fn new() -> ActivityId<Self> {
        let this = nuts::new_domained_activity(Self, &DefaultDomain);

        this.subscribe_domained(Self::on_batch_start);
        this.subscribe_domained(Self::on_batch_done);

        this
    }

    /// add new batch execution plan
    fn on_batch_start(&mut self, state: &mut DomainState, batch_start: &msg::BatchStart) {
        let state: &SimulationState = state.get();
        let mut state = state.borrow_mut();

        let batch: Batch = batch_start.clone().into();

        info!(time = %state.time, batch.id = batch.id, batch.done.len = batch.done.len(), batch.past_due.len = batch.past_due.len(), "BatchStart");

        state.workers[batch.id].batch_start(&batch);
        let done = batch.interval.ub();
        state
            .backend
            .post_event(done, msg::BatchDone { batch });
    }

    /// release the slot when the batch is done
    fn on_batch_done(&mut self, state: &mut DomainState, msg: &msg::BatchDone) {
        let state: &SimulationState = state.get();
        let mut state = state.borrow_mut();

        info!(
            time = %state.time,
            batch.id = msg.batch.id,
            batch.started = %msg.batch.started(),
            "stop batch on worker",
        );
        state.workers[msg.batch.id].batch_done(&msg.batch);
    }
}

impl From<msg::BatchStart> for Batch {
    fn from(bs: msg::BatchStart) -> Self {
        // calculate latency
        let latency = bs
            .jobs
            .iter()
            .map(|j| j.length.value())
            .reduce(|a, b| if a < b { b } else { a })
            .expect("Batch can not be empty");
        // split jobs into done and past due
        let finished = bs.when + latency;
        let (past_due, done) = bs
            .jobs
            .into_iter()
            .partition(|job| job.missed_deadline(finished));
        // save info
        Self {
            id: bs.which,
            interval: TimeInterval(bs.when, latency),
            done,
            past_due,
        }
    }
}

/// handles some common functionality for schedulers, and adapts any `impl Scheduler` types to the simulator.
/// handles job past due, and job admission.
struct SchedulerController {
    scheduler: Box<dyn schedulers::Scheduler + 'static>,
    next_wakeup: Time,
}

impl SchedulerController {
    pub fn new(scheduler: Box<dyn schedulers::Scheduler + 'static>) -> ActivityId<SchedulerController> {
        let this = nuts::new_domained_activity(
            Self {
                scheduler,
                next_wakeup: Default::default(),
            },
            &DefaultDomain,
        );
        this.subscribe_domained(Self::on_wake_up);
        this.subscribe_domained(Self::on_incoming_jobs);
        this.subscribe_domained(Self::on_batch_done);

        this
    }

    // schedule next wake up event based on earliest past due
    fn schedule_wake_up(&mut self, state: &Simulation) {
        if let Some(time) = state
            .pending_jobs
            .iter()
            .filter_map(|job| job.deadline)
            .min()
        {
            if time != self.next_wakeup {
                self.next_wakeup = time;
                state
                    .backend
                    .post_event(time, msg::WakeUpSchedulerController);
            }
        }
    }

    // handles job past due
    fn on_wake_up(&mut self, state: &mut DomainState, _: &msg::WakeUpSchedulerController) {
        let state: &SimulationState = state.get();
        let mut state = state.borrow_mut();

        // handle past due jobs
        state.pending_jobs = {
            let time = state.time;
            let (past_due, pending_jobs): (VecDeque<_>, _) = state
                .pending_jobs
                .drain(..)
                .partition(|j| j.missed_deadline(time));

            assert_eq!(state.pending_jobs.len(), 0);
            info!(
                past_due.len = past_due.len(),
                pending_jobs.len = pending_jobs.len(),
                "past_due stats"
            );

            if !past_due.is_empty() {
                state
                    .backend
                    .post_event(state.time, msg::PastDue { jobs: past_due.into() });
            }
            pending_jobs
        };

        self.scheduler.on_wake_up(&mut state);

        // re-schedule next wake up event
        self.schedule_wake_up(&state);
    }

    // admit job into pending job,
    // and forward to `impl Scheduler`
    fn on_incoming_jobs(&mut self, state: &mut DomainState, msg: &msg::IncomingJobs) {
        let state: &SimulationState = state.get();
        let mut state = state.borrow_mut();

        // new jobs coming in, accept as pending jobs
        let time = state.time;
        state.pending_jobs.extend(
            msg.jobs
                .iter()
                .map(|ij| ij.clone().into_job(time)),
        );
        info!(
            time = %state.time,
            pending_jobs.len = state.pending_jobs.len(),
            "accepted {} incoming jobs",
            msg.jobs.len()
        );

        // re-schedule next wake up
        self.schedule_wake_up(&state);

        self.scheduler.on_incoming_jobs(&mut state, msg);
    }

    // forward to `impl Scheduler`
    fn on_batch_done(&mut self, state: &mut DomainState, msg: &msg::BatchDone) {
        let state: &SimulationState = state.get();
        let mut state = state.borrow_mut();

        info!(time = %state.time, "forward to scheduler");

        self.scheduler.on_batch_done(&mut state, msg);
    }
}

/// The front-end of the various controllers, glue them together
pub(crate) struct Simulator {
    backend: ActivityId<SimulationController>,
    state: Rc<RefCell<Simulation>>,
}

impl Simulator {
    /// Setup simulator on the current thread
    pub fn new(
        incoming_jobs: Incoming<'static>,
        scheduler: Box<dyn schedulers::Scheduler + 'static>,
        workers: Vec<Worker>,
    ) -> Simulator {
        // the global simulation controller, for event dispatching, also handles admitting jobs
        let backend = SimulationController::new();

        // the incoming controller posts incoming job events according to the Incoming iter
        let incoming_controller = IncomingController::new(incoming_jobs);

        // the worker controller tracks execution and generates batch done events
        let worker_controller = WorkerController::new();

        // the scheduler controller drives the `impl Scheduler` scheduler
        let scheduler_controller = SchedulerController::new(scheduler);

        // create states
        let state = Rc::new(RefCell::new(Simulation {
            processed_events: Default::default(),
            future_events: Default::default(),
            time: Default::default(),
            pending_jobs: Default::default(),
            workers,
            backend,
            incoming_controller,
            worker_controller,
            scheduler_controller,
        }));
        nuts::store_to_domain(&DefaultDomain, state.clone());

        let this = Simulator { backend, state };

        // seed the events
        backend.post_event(0.0, msg::WakeUpIncomingController);
        backend.post_event(0.0, msg::WakeUpWorkerController);
        backend.post_event(0.0, msg::WakeUpSchedulerController);

        this
    }

    pub fn step(&self) {
        self.backend.private_message(RunStep);
    }

    fn is_end(&self, until: &EndCondition) -> bool {
        let state = self.state.borrow();
        match until {
            EndCondition::Time { max: t } => state.time > *t,
            EndCondition::NoEvents => state.future_events.is_empty(),
        }
    }

    pub fn run(&self, until: EndCondition) {
        while !self.is_end(&until) {
            self.step();
        }
    }

    pub fn processed_events(&self) -> Vec<Event> {
        let state = self.state.borrow();
        state.processed_events.clone()
    }
}
