#![feature(generators, generator_trait, backtrace, control_flow_enum)]

use desim::EndCondition;
use rand_seeder::{Seeder, SipRng};

use crate::simulator::build_simulation;
use crate::utils::prelude::*;

mod config;
mod incoming;
mod output;
mod schedulers;
mod simulator;
mod types;
pub mod utils;

#[derive(Debug, serde::Deserialize, serde::Serialize)]
struct SimConfig {
    seed: Option<String>,
    incoming: incoming::IncomingJobConfig,
    scheduler: schedulers::SchedulerConfig,
}

pub fn run_sim() -> Result<()> {
    let cfg: SimConfig = config().fetch()?;
    // setup rng
    let rng: SipRng = Seeder::from(cfg.seed.as_deref().unwrap_or("stripy zebra")).make_rng();

    // setup incoming jobs
    let incoming_jobs = incoming::from_config(rng.clone(), &cfg.incoming)?;

    // setup scheduler
    let scheduler = schedulers::from_config(rng, &cfg.scheduler)?;

    // build simulator
    let mut sim = build_simulation(scheduler, incoming_jobs);

    // run!
    let _g = info_span!("sim_run").entered();
    sim = sim.run(EndCondition::NoEvents);

    output::render_chrome_trace(sim.processed_events())?;

    Ok(())
}
