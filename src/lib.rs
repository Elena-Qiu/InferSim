#![feature(generators, generator_trait, backtrace, control_flow_enum)]

use std::fmt;

use desim::EndCondition;
use rand_seeder::{Seeder, SipRng};

mod incoming;
mod output;
mod schedulers;
mod simulator;
pub mod utils;

use crate::simulator::build_simulation;
use utils::prelude::*;

/// Incoming job, not yet accepted by the system
#[derive(Debug, Clone)]
pub struct IncomingJob {
    /// Job ID
    pub id: usize,
    /// Inference length
    pub length: f64,
    /// time budget
    pub budget: Option<f64>,
}

impl fmt::Display for IncomingJob {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.budget {
            Some(b) => write!(f, "IncomingJob({}, {:.2}, {:.2})", self.id, self.length, b),
            None => write!(f, "IncomingJob({}, {:.2}, None)", self.id, self.length),
        }
    }
}

impl IncomingJob {
    pub fn into_job(self, admitted: f64) -> Job {
        Job {
            id: self.id,
            admitted,
            length: self.length,
            deadline: self.budget.map(|b| b + admitted),
        }
    }
}

/// A job admitted in the system
#[derive(Debug, Clone)]
pub struct Job {
    pub id: usize,
    pub admitted: f64,
    pub length: f64,
    /// deadline, absolute
    pub deadline: Option<f64>,
}

impl Job {
    pub fn missed_deadline(&self, time: f64) -> bool {
        self.deadline.map(|d| d > time).unwrap_or(false)
    }
}

impl fmt::Display for Job {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.deadline {
            Some(d) => write!(f, "Job({}, @{:.2}<{:.2}<{:.2})", self.id, self.admitted, self.length, d),
            None => write!(f, "Job({}, @{:.2}<{:.2}<None)", self.id, self.admitted, self.length),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Batch {
    pub id: usize,
    pub jobs: Vec<Job>,
    pub started: f64,
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
            .map(|j| j.length)
            .reduce(f64::max)
            .expect("Batch can not be empty")
    }
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
struct SimConfig {
    seed: Option<String>,
    incoming: incoming::IncomingJobConfig,
}

pub fn run_sim() -> Result<()> {
    let cfg: SimConfig = config().fetch()?;
    // setup rng
    let mut rng: SipRng = Seeder::from(cfg.seed.as_deref().unwrap_or("stripy zebra")).make_rng();

    // setup incoming jobs
    let incoming_jobs = incoming::from_config(rng.clone(), &cfg.incoming)?;

    // setup scheduler
    let scheduler = schedulers::Random::new(rng.clone(), 2);

    // build simulator
    let mut sim = build_simulation(scheduler, incoming_jobs);

    // run!
    let _g = info_span!("sim_run").entered();
    sim = sim.run(EndCondition::NoEvents);

    output::render_chrome_trace(sim.processed_events())?;

    Ok(())
}
