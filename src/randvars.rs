use rand::distributions::{Distribution, Uniform};
use rand::Rng;
use rand_distr::{Exp, LogNormal, Normal, Poisson, StandardNormal};

use crate::types::Observation;
use crate::utils::{prelude, BoxIterator, IntoBoxIter};

#[derive(Debug, Copy, Clone, serde::Deserialize, serde::Serialize)]
#[serde(tag = "type")]
pub enum RandomVariable<T> {
    Constant(T),
    Uniform { low: T, high: T },
    Normal { mean: T, std_dev: T },
    LogNormal { mean: T, std_dev: T },
    Poisson { lambda: T },
    Exp { lambda: T, mean: T, scale: T },
}

impl RandomVariable<f64> {
    pub fn quantile(&self, percentage: f64) -> f64 {
        // todo
        todo!()
    }
}

impl RandomVariable<f64>
where
    StandardNormal: Distribution<f64>,
{
    pub fn sample_iter<'a, W: Copy + From<f64>>(
        &self,
        rng: impl Rng + 'a,
    ) -> prelude::Result<BoxIterator<'a, Observation<W, f64>>> {
        let dist = *self;
        let iter: BoxIterator<_> = match self {
            RandomVariable::Uniform { low, high } => Uniform::new(low.min(*high), high.max(*low))
                .sample_iter(rng)
                .map(move |v| Observation::new(v, dist))
                .into_boxed(),
            RandomVariable::Normal { mean, std_dev } => Normal::new(*mean, *std_dev)?
                .sample_iter(rng)
                .map(move |v| Observation::new(v, dist))
                .into_boxed(),
            RandomVariable::LogNormal { mean, std_dev } => LogNormal::new(*mean, *std_dev)?
                .sample_iter(rng)
                .map(move |v| Observation::new(v, dist))
                .into_boxed(),
            RandomVariable::Poisson { lambda } => Poisson::new(*lambda)?
                .sample_iter(rng)
                .map(move |v| Observation::new(v, dist))
                .into_boxed(),
            RandomVariable::Exp { lambda, mean, scale } => {
                let mean = *mean;
                let scale = *scale;
                Exp::new(*lambda)?
                    .sample_iter(rng)
                    .map(move |s| s * scale + mean)
                    .map(move |v| Observation::new(v, dist))
                    .into_boxed()
            }
            RandomVariable::Constant(v) => std::iter::repeat(*v)
                .map(move |v| Observation::new(v, dist))
                .into_boxed(),
        };
        Ok(iter)
    }
}
