#![feature(generators, generator_trait, backtrace, control_flow_enum)]

use std::fmt;

use desim::EndCondition;
use rand::distributions;
use rand_seeder::{Seeder, SipRng};

mod incoming;
mod schedulers;
mod simulator;
pub mod utils;

use crate::simulator::build_simulation;
use utils::prelude::*;

#[derive(Debug, Clone)]
pub struct Job {
    /// Inference length
    length: f64,
}

impl fmt::Display for Job {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Job {{ length: {:.2} }}", self.length)
    }
}

impl Job {
    pub fn new(length: f64) -> Self {
        Job { length }
    }

    pub fn latency(&self) -> f64 {
        self.length
    }
}

#[derive(Debug, Clone)]
pub struct Batch {
    pub jobs: Vec<Job>,
}

impl fmt::Display for Batch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Batch {{ jobs.len: {} }}", self.jobs.len())
    }
}

impl Batch {
    /// the batch processing time is the max of all jobs in the batch
    pub fn latency(&self) -> f64 {
        self.jobs
            .iter()
            .map(|j| j.latency())
            .reduce(f64::max)
            .expect("Batch can not be empty")
    }
}

pub fn run_sim() -> Result<()> {
    let mut rng: SipRng = Seeder::from("stripy zebra").make_rng();

    let scheduler = schedulers::FIFO::new(2);
    let incoming_jobs = incoming::one_batch(&mut rng, 0.0, 10, distributions::Standard);
    let sim = build_simulation(scheduler, incoming_jobs);

    let _g = info_span!("sim_run").entered();
    sim.run(EndCondition::NoEvents);

    Ok(())
}
