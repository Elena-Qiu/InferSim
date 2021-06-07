use std::collections::VecDeque;

use crate::schedulers::Scheduler;
use crate::simulator::SystemState;
use crate::types::{Batch, Job, Time};

#[derive(Debug)]
pub struct Nexus;

impl Scheduler for Nexus {
    fn on_new_jobs(&mut self, _now: Time, _pending_jobs: &mut VecDeque<Job>) -> Vec<SystemState> {
        todo!()
    }

    fn on_batch_done(&mut self, _now: Time, _batch: &Batch, _pending_jobs: &mut VecDeque<Job>) -> Vec<SystemState> {
        todo!()
    }
}
