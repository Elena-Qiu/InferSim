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
use crate::types::{Batch, IncomingJob, Job, Time, TimeInterval};
use crate::utils::prelude::*;
use crate::workers::Worker;
use crate::EndCondition;

mod msg;

/// Internal event to drive the SimulationController
struct RunStep;

/// Events processed by various controllers
#[derive(Debug, Clone, Ord, PartialOrd, Eq, PartialEq, Display)]
#[display("@{time:.2} -> {message}")]
struct Event {
    pub time: Time,
    pub message: Message,
}

macro_rules! define_message {
    ( $( $msg:ident ),+ ) => {

        #[derive(Debug, Clone, Display, Educe)]
        #[educe(PartialEq, Eq, PartialOrd, Ord)]
        #[display("{}")]
        enum Message {
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
    WakeUpScheduler,
    WakeUpWorkerController,
    WakeUpIncomingController,
    IncomingJobs,
    PastDue,
    BatchStart,
    BatchDone
];

struct Simulation {
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
            let _s = debug_span!("event", event.time = %event.time, event.message = %event.message).entered();
            state.time = event.time;
            // handle past due jobs
            state.pending_jobs = {
                let time = state.time;
                let (past_due, pending_jobs): (VecDeque<_>, _) = state
                    .pending_jobs
                    .drain(..)
                    .partition(|j| j.missed_deadline(time));
                if !past_due.is_empty() {
                    state
                        .backend
                        .post_event(state.time, msg::PastDue { jobs: past_due.into() });
                }
                pending_jobs
            };
            // record the event now because publishing will consume the event
            state.processed_events.push(event.clone());
            // dispatch events to other components
            match event.message {
                Message::WakeUpScheduler(inner) => nuts::publish(inner),
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
        info!(%event, "push event");
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

        let batch = Batch {
            id: batch_start.which,
            jobs: batch_start.jobs.clone(),
            started: batch_start.when,
        };

        state.workers[batch.id].batch_start(&batch);

        let TimeInterval(_, done) = batch.to_interval();
        state
            .backend
            .post_event(done, msg::BatchDone { batch });
    }

    /// release the slot when the batch is done
    fn on_batch_done(&mut self, state: &mut DomainState, msg: &msg::BatchDone) {
        let state: &SimulationState = state.get();
        let mut state = state.borrow_mut();

        state.workers[msg.batch.id].batch_done(&msg.batch);
    }
}

/// The front-end of the various controllers, glue them together
struct Simulator {
    backend: ActivityId<SimulationController>,
    state: Rc<RefCell<Simulation>>,
}

impl Simulator {
    /// Setup simulator on the current thread
    pub fn new(incoming_jobs: Incoming<'static>, workers: Vec<Worker>) -> Simulator {
        // the global simulation controller, for event dispatching, also handles admitting jobs
        let backend = SimulationController::new();

        // the incoming controller posts incoming job events according to the Incoming iter
        let incoming_controller = IncomingController::new(incoming_jobs);

        // the worker controller tracks execution and generates batch done events
        let worker_controller = WorkerController::new();

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
        }));
        nuts::store_to_domain(&DefaultDomain, state.clone());

        let this = Simulator { backend, state };

        // seed the events
        backend.post_event(0.0, msg::WakeUpIncomingController);
        backend.post_event(0.0, msg::WakeUpWorkerController);
        backend.post_event(0.0, msg::WakeUpScheduler);

        this
    }

    pub fn step(&self) {
        self.backend.private_message(RunStep);
    }

    fn is_end(&self, until: &EndCondition) -> bool {
        let state = self.state.borrow();
        match until {
            EndCondition::Time(t) => state.time > *t,
            EndCondition::NoEvents => state.future_events.is_empty(),
        }
    }

    pub fn run(&self, until: EndCondition) {
        while !self.is_end(&until) {
            self.step();
        }
    }
}
