#![feature(generators, generator_trait, backtrace, control_flow_enum)]
#![feature(total_cmp)]

use rand_seeder::{Seeder, SipRng};

use crate::simulator::schedule_loop;
use crate::types::Time;
use crate::utils::prelude::*;

mod config;
mod incoming;
mod output;
mod randvars;
mod schedulers;
mod simulator;
mod types;
pub mod utils;
mod workers;

#[derive(Debug, Clone, Copy, serde::Deserialize, serde::Serialize)]
enum EndCondition {
    NoEvents,
    Time(Time),
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
struct SimConfig {
    seed: Option<String>,
    incoming: incoming::IncomingConfig,
    scheduler: schedulers::SchedulerConfig,
    workers: Vec<workers::WorkerConfig>,
    until: EndCondition,
}

pub fn run_sim() -> Result<()> {
    let _g = info_span!("sim").entered();

    let cfg: SimConfig = config().fetch()?;
    let events = {
        let _g = info_span!("run").entered();

        // setup rng
        let rng: SipRng = Seeder::from(cfg.seed.as_deref().unwrap_or("stripy zebra")).make_rng();

        // setup incoming jobs
        let incoming_jobs = incoming::from_config(rng.clone(), &cfg.incoming)?;

        // setup workers
        let workers = workers::from_config(&cfg.workers);

        // setup scheduler
        let scheduler = schedulers::from_config(&cfg.scheduler, rng, workers)?;

        // run!
        schedule_loop(scheduler, incoming_jobs, cfg.until)
    };

    // outputs
    {
        let _g = info_span!("output").entered();
        output::render_chrome_trace(&events)?;
        output::render_job_trace(&events)?;
    }

    Ok(())
}
