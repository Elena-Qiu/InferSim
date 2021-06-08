#![feature(generators, generator_trait, backtrace, control_flow_enum)]
#![feature(total_cmp)]
#![feature(vecdeque_binary_search)]

use rand_seeder::{Seeder, SipRng};

use crate::types::Time;
use crate::utils::prelude::*;

mod config;
mod incoming;
mod output;
pub mod randvars;
mod sim;
mod types;
pub mod utils;
pub mod workers;

#[derive(Debug, Clone, Copy, serde::Deserialize, serde::Serialize)]
#[serde(tag = "type")]
enum EndCondition {
    NoEvents,
    Time { max: Time },
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
struct SimConfig {
    seed: Option<String>,
    incoming: incoming::IncomingConfig,
    scheduler: sim::schedulers::SchedulerConfig,
    workers: Vec<workers::WorkerConfig>,
    until: EndCondition,
}

pub fn run_sim() -> Result<()> {
    let cfg: SimConfig = config().fetch()?;
    let events = {
        // setup rng
        let rng: SipRng = Seeder::from(cfg.seed.as_deref().unwrap_or("stripy zebra")).make_rng();

        // setup incoming jobs
        let incoming_jobs = incoming::from_config(rng.clone(), &cfg.incoming)?;

        // setup workers
        let workers = workers::from_config(&cfg.workers);

        // setup scheduler
        let scheduler = sim::schedulers::from_config(&cfg.scheduler, rng)?;

        // run!
        let sim = sim::Simulator::new(incoming_jobs, scheduler, workers);
        sim.run(cfg.until);
        sim.processed_events()
    };

    // outputs
    {
        output::render_chrome_trace(events.iter())?;
        output::render_job_trace(events.iter())?;
    }

    Ok(())
}
