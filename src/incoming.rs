use rand::Rng;
use rand_distr::{Distribution, Exp, LogNormal, Normal, Poisson, StandardNormal, Uniform};

use crate::utils::prelude::*;
use crate::IncomingJob;

pub fn from_config<'a>(
    rng: impl Rng + 'a,
    cfg: &'a IncomingJobConfig,
) -> Result<impl Iterator<Item = (f64, impl Iterator<Item = IncomingJob> + 'a)>> {
    let incoming = match cfg {
        IncomingJobConfig::OneBatch { delay, n_jobs, spec } => {
            let jobs = spec
                .length
                .sample_iter(rng)?
                .zip(0..*n_jobs)
                .map(move |(length, id)| IncomingJob {
                    id,
                    length,
                    budget: spec.budget,
                });
            std::iter::once((*delay, jobs))
        }
    };
    Ok(incoming)
}

// ====== Config to rand ======
type BoxedDistIter<'a, T> = Box<dyn Iterator<Item = T> + 'a>;

impl RandomVariable<f64>
where
    StandardNormal: Distribution<f64>,
{
    pub fn sample_iter<'a>(&self, rng: impl Rng + 'a) -> Result<BoxedDistIter<'a, f64>> {
        let iter: BoxedDistIter<f64> = match self {
            RandomVariable::Uniform { low, high } => {
                Box::new(Uniform::new(low.min(*high), high.max(*low)).sample_iter(rng))
            }
            RandomVariable::Normal { mean, std_dev } => Box::new(Normal::new(*mean, *std_dev)?.sample_iter(rng)),
            RandomVariable::LogNormal { mean, std_dev } => Box::new(LogNormal::new(*mean, *std_dev)?.sample_iter(rng)),
            RandomVariable::Poisson { lambda } => Box::new(Poisson::new(*lambda)?.sample_iter(rng)),
            RandomVariable::Exp { lambda } => Box::new(Exp::new(*lambda)?.sample_iter(rng)),
        };
        Ok(iter)
    }
}

// ====== Config ======

#[derive(Debug, serde::Deserialize)]
#[serde(tag = "type")]
pub enum IncomingJobConfig {
    OneBatch {
        /// the initial delay
        delay: f64,
        /// Number of batch
        n_jobs: usize,
        /// spec to generate jobs
        spec: JobSpec,
    },
}

#[derive(Debug, serde::Deserialize)]
pub struct JobSpec {
    length: RandomVariable<f64>,
    /// SLO budget
    /// If none, no deadline
    budget: Option<f64>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(tag = "type")]
pub enum RandomVariable<T> {
    Uniform { low: T, high: T },
    Normal { mean: T, std_dev: T },
    LogNormal { mean: T, std_dev: T },
    Poisson { lambda: T },
    Exp { lambda: T },
}
