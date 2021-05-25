use itertools::Itertools as _;
use rand::Rng;
use rand_distr::{Distribution, Exp, LogNormal, Normal, Poisson, StandardNormal, Uniform};

use crate::types::{Duration, IncomingJob, Time};
use crate::utils::prelude::*;
use crate::utils::{BoxIterator, IntoBoxIter};

pub struct Incoming<'a>(BoxIterator<'a, (Duration, Vec<IncomingJob>)>);

impl<'a> Iterator for Incoming<'a> {
    type Item = (Duration, Vec<IncomingJob>);

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}

impl<'a> Incoming<'a> {
    /// Given one incoming iter, do necessary book keeping to output absolute incoming batch time
    pub fn into_absolute(self) -> IncomingAbsolute<'a> {
        IncomingAbsolute(Time(0.0), self.0)
    }
}

pub struct IncomingAbsolute<'a>(Time, BoxIterator<'a, (Duration, Vec<IncomingJob>)>);

impl<'a> Iterator for IncomingAbsolute<'a> {
    type Item = (Time, Vec<IncomingJob>);

    fn next(&mut self) -> Option<Self::Item> {
        let (delay, batch) = self.1.next()?;
        self.0 += delay;
        Some((self.0, batch))
    }
}

fn one_batch<'a>(
    rng: impl Rng + 'a,
    base_id: usize,
    delay: Duration,
    n_jobs: usize,
    spec: &'a JobSpec,
) -> Result<Incoming> {
    let batch = spec
        .length
        .sample_iter(rng)?
        .zip(0..n_jobs)
        .map(move |(length, id)| IncomingJob {
            id: id + base_id,
            length: Duration(length),
            budget: spec.budget.map(Duration),
        })
        .collect();
    Ok(Incoming(std::iter::once((delay, batch)).into_boxed()))
}

/// Combine multiple model incoming requests into one stream of jobs
fn combined<'a>(models: impl IntoIterator<Item = Incoming<'a>>) -> Incoming<'a> {
    let models = models
        .into_iter()
        .map(|it| it.into_absolute())
        // merge sort by absolute time
        .kmerge_by(|a, b| a.0 < b.0)
        // convert back to delay based
        .scan(Time(0.0), |cur_time, (time, batch)| {
            let delay = time - *cur_time;
            *cur_time = time;
            Some((delay, batch))
        });
    Incoming(models.into_boxed())
}

impl IncomingJobConfig {
    pub fn as_iter<'a>(&'a self, rng: impl Rng + 'a, base_id: usize) -> Result<Incoming<'a>> {
        let it = match self {
            IncomingJobConfig::OneBatch { delay, n_jobs, spec } => one_batch(rng, base_id, *delay, *n_jobs, spec)?,
            _ => panic!("not implemented"),
        };
        Ok(it)
    }
}

pub fn from_config<'a>(rng: impl Rng + Clone + 'a, cfg: &'a IncomingConfig) -> Result<Incoming> {
    let incoming = match cfg {
        IncomingConfig::Array(models) => {
            let models: Vec<_> = models
                .iter()
                .zip((0..).step_by(1000000usize))
                .map(move |(m, base_id)| m.as_iter(rng.clone(), base_id))
                .try_collect()?;
            combined(models)
        }
        IncomingConfig::Single(model) => model.as_iter(rng, 0)?,
    };
    Ok(incoming)
}

// ====== Config to rand ======
impl RandomVariable<f64>
where
    StandardNormal: Distribution<f64>,
{
    pub fn sample_iter<'a>(&self, rng: impl Rng + 'a) -> Result<BoxIterator<'a, f64>> {
        let iter: BoxIterator<f64> = match self {
            RandomVariable::Uniform { low, high } => Uniform::new(low.min(*high), high.max(*low))
                .sample_iter(rng)
                .into_boxed(),
            RandomVariable::Normal { mean, std_dev } => Normal::new(*mean, *std_dev)?
                .sample_iter(rng)
                .into_boxed(),
            RandomVariable::LogNormal { mean, std_dev } => LogNormal::new(*mean, *std_dev)?
                .sample_iter(rng)
                .into_boxed(),
            RandomVariable::Poisson { lambda } => Poisson::new(*lambda)?
                .sample_iter(rng)
                .into_boxed(),
            RandomVariable::Exp { lambda, mean, scale } => {
                let mean = *mean;
                let scale = *scale;
                Exp::new(*lambda)?
                    .sample_iter(rng)
                    .map(move |s| s * scale + mean)
                    .into_boxed()
            }
            RandomVariable::Constant(v) => std::iter::repeat(*v).into_boxed(),
        };
        Ok(iter)
    }
}

// ====== Config ======
#[derive(Debug, serde::Deserialize, serde::Serialize)]
#[serde(untagged)]
pub enum IncomingConfig {
    Array(Vec<IncomingJobConfig>),
    Single(IncomingJobConfig),
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
#[serde(tag = "type")]
pub enum IncomingJobConfig {
    OneBatch {
        /// the initial delay
        delay: Duration,
        /// Number of batch
        n_jobs: usize,
        /// spec to generate jobs
        spec: JobSpec,
    },
    Rate {
        /// Number of jobs per unit of time
        per: usize,
        /// Unit of time
        unit: f64,
        /// Spec of generated jobs
        spec: JobSpec,
    },
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct JobSpec {
    length: RandomVariable<f64>,
    /// SLO budget
    /// If none, no deadline
    budget: Option<f64>,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
#[serde(tag = "type")]
pub enum RandomVariable<T> {
    Constant(T),
    Uniform { low: T, high: T },
    Normal { mean: T, std_dev: T },
    LogNormal { mean: T, std_dev: T },
    Poisson { lambda: T },
    Exp { lambda: T, mean: T, scale: T },
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand_seeder::{Seeder, SipRng};

    fn get_rng() -> SipRng {
        Seeder::from("stripy zebra").make_rng()
    }

    #[test]
    fn one_batch() {
        let mut model = super::one_batch(
            get_rng(),
            0,
            Duration(5.),
            1,
            &JobSpec {
                length: RandomVariable::Constant(10.),
                budget: None,
            },
        )
        .unwrap();

        let (delay, batch) = model.next().unwrap();
        assert_eq!(model.next(), None);
        assert_eq!(delay, Duration(5.));
        assert_eq!(
            batch,
            vec![IncomingJob {
                id: 0,
                length: Duration(10.),
                budget: None
            }]
        );
    }

    #[test]
    fn combined() {
        let rng = get_rng();
        let a = super::one_batch(
            rng.clone(),
            0,
            Duration(5.),
            1,
            &JobSpec {
                length: RandomVariable::Constant(10.),
                budget: None,
            },
        )
        .unwrap();
        let b = super::one_batch(
            rng.clone(),
            10,
            Duration(2.),
            1,
            &JobSpec {
                length: RandomVariable::Constant(20.),
                budget: None,
            },
        )
        .unwrap();

        let mut c = super::combined(vec![a, b]);
        assert_eq!(
            c.next(),
            Some((
                Duration(2.),
                vec![IncomingJob {
                    id: 10,
                    length: Duration(20.),
                    budget: None
                }]
            ))
        );
        assert_eq!(
            c.next(),
            Some((
                Duration(3.),
                vec![IncomingJob {
                    id: 0,
                    length: Duration(10.),
                    budget: None
                }]
            ))
        );
        assert_eq!(c.next(), None);
    }
}
