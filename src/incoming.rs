use itertools::Itertools as _;
use rand::distributions::Open01;
use rand::Rng;

use crate::randvars::RandomVariable;
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
    spec: &JobSpec,
) -> Result<Incoming<'a>> {
    let budget = spec.budget.map(Duration);
    let batch = spec
        .length
        .sample_iter(rng)?
        .zip(0..n_jobs)
        .map(move |(length, id)| IncomingJob {
            id: id + base_id,
            length,
            budget,
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

fn rate<'a>(
    rng: impl Clone + Rng + 'a,
    base_id: usize,
    per: usize,
    unit: Duration,
    bursty: bool,
    spec: &JobSpec,
) -> Result<Incoming<'a>> {
    info!(per, %unit, bursty, "using incoming=rate");
    let budget = spec.budget.map(Duration);
    let mut it = spec
        .length
        .sample_iter(rng.clone())?
        .zip(base_id..)
        .map(move |(length, id)| IncomingJob { id, length, budget });

    let batches = std::iter::repeat_with(move || (&mut it).take(per).collect());
    let it = if bursty {
        rng.sample_iter(Open01)
            .map(move |c: f64| unit * -c.ln())
            .zip(batches)
            .into_boxed()
    } else {
        std::iter::repeat(unit).zip(batches).into_boxed()
    };
    Ok(Incoming(it))
}

impl IncomingJobConfig {
    pub fn as_iter<'a>(&self, rng: impl Rng + Clone + 'a, base_id: usize) -> Result<Incoming<'a>> {
        let it = match self {
            IncomingJobConfig::OneBatch { delay, n_jobs, spec } => one_batch(rng, base_id, *delay, *n_jobs, spec)?,
            IncomingJobConfig::Rate {
                per,
                unit,
                spec,
                bursty,
            } => rate(rng, base_id, *per, *unit, *bursty, spec)?,
        };
        Ok(it)
    }
}

pub fn from_config<'a, 'b>(rng: impl Rng + Clone + 'a, cfg: &'b IncomingConfig) -> Result<Incoming<'a>> {
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
        unit: Duration,
        /// Spec of generated jobs
        spec: JobSpec,
        /// Whether to apply a random exponential transform on the unit duration, creating a bursty effect
        #[serde(default)]
        bursty: bool,
    },
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct JobSpec {
    length: RandomVariable,
    /// SLO budget
    /// If none, no deadline
    budget: Option<f64>,
}

#[cfg(test)]
mod tests {
    use rand_seeder::{Seeder, SipRng};

    use super::*;
    use crate::randvars::Observation;

    fn get_rng() -> SipRng {
        Seeder::from("stripy zebra").make_rng()
    }

    #[test]
    fn one_batch() {
        let rand_length = RandomVariable::constant(10.);
        let model = super::one_batch(
            get_rng(),
            0,
            Duration(5.),
            1,
            &JobSpec {
                length: rand_length.clone(),
                budget: None,
            },
        )
        .unwrap();

        let batches = model.collect_vec();
        assert_eq!(
            batches,
            vec![(
                Duration(5.),
                vec![IncomingJob {
                    id: 0,
                    length: Observation::new(10., rand_length),
                    budget: None,
                }]
            )]
        );
    }

    #[test]
    fn combined() {
        let rng = get_rng();
        let rand_length = RandomVariable::constant(10.);
        let rand_length2 = RandomVariable::constant(20.);
        let a = super::one_batch(
            rng.clone(),
            0,
            Duration(5.),
            1,
            &JobSpec {
                length: rand_length.clone(),
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
                length: rand_length2.clone(),
                budget: None,
            },
        )
        .unwrap();

        let c = super::combined(vec![a, b]);
        let batches = c.collect_vec();
        assert_eq!(
            batches,
            vec![
                (
                    Duration(2.),
                    vec![IncomingJob {
                        id: 10,
                        length: Observation::new(20., rand_length2),
                        budget: None
                    }]
                ),
                (
                    Duration(3.),
                    vec![IncomingJob {
                        id: 0,
                        length: Observation::new(10., rand_length),
                        budget: None
                    }]
                ),
            ]
        );
    }

    #[test]
    fn rate() {
        let rng = get_rng();
        let rand_length = RandomVariable::constant(10.);
        let model = super::rate(
            rng,
            0,
            2,
            Duration(1.),
            false,
            &JobSpec {
                length: rand_length.clone(),
                budget: None,
            },
        )
        .unwrap();

        // this is a infinite model, so take a few
        let batches = model.take(2).collect_vec();
        assert_eq!(
            batches,
            vec![
                (
                    Duration(1.),
                    vec![
                        IncomingJob {
                            id: 0,
                            length: Observation::new(10., rand_length.clone()),
                            budget: None
                        },
                        IncomingJob {
                            id: 1,
                            length: Observation::new(10., rand_length.clone()),
                            budget: None
                        },
                    ]
                ),
                (
                    Duration(1.),
                    vec![
                        IncomingJob {
                            id: 2,
                            length: Observation::new(10., rand_length.clone()),
                            budget: None
                        },
                        IncomingJob {
                            id: 3,
                            length: Observation::new(10., rand_length),
                            budget: None
                        },
                    ]
                ),
            ]
        );
    }
}
