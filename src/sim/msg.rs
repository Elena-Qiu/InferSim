use crate::types::{Batch, IncomingJob, Job, Time};

/// Wake up scheduler
#[derive(Debug, Clone, Copy)]
pub struct WakeUpSchedulerController;

/// Wake up worker controller
#[derive(Debug, Clone, Copy)]
pub struct WakeUpWorkerController;

/// Wake up incoming controller
#[derive(Debug, Clone, Copy)]
pub struct WakeUpIncomingController;

// events may be generated by the incoming controller

/// State/event injected by scheduler_process for all incoming jobs
#[derive(Debug, Clone)]
pub struct IncomingJobs {
    pub jobs: Vec<IncomingJob>,
}

/// Some jobs are dropped because their deadlines are missed
#[derive(Debug, Clone)]
pub struct PastDue {
    pub jobs: Vec<Job>,
}

// events may be generated by the scheduler

/// schedule a batch on a worker
#[derive(Debug, Clone)]
pub struct BatchStart {
    /// on which worker
    pub which: usize,
    /// when
    pub when: Time,
    /// the content of the batch
    pub jobs: Vec<Job>,
}

// events generated by the worker controller

#[derive(Debug, Clone)]
pub struct BatchDone {
    pub batch: Batch,
}
