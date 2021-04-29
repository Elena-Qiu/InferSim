#![feature(generators, generator_trait, backtrace)]

use rand::distributions;
use rand_seeder::{Seeder, SipRng};

mod incoming;
mod schedulers;
mod simulator;
pub mod utils;

use crate::simulator::build_simulation;
use desim::EndCondition;
use utils::logging::prelude::*;
use utils::Result;

#[derive(Debug, Clone)]
pub struct Job {
    /// Inference length
    length: f64,
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
    info!("Running sim");

    let mut rng: SipRng = Seeder::from("stripy zebra").make_rng();

    let scheduler = schedulers::FIFO::new(2);
    let incoming_jobs = incoming::one_batch(&mut rng, 0.0, 10, distributions::Standard);
    let mut sim = build_simulation(scheduler, incoming_jobs);

    sim.run(EndCondition::NoEvents);

    Ok(())
}
