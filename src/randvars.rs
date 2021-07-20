use std::ops::Deref;

use approx::ulps_eq;
use rand::{distributions::Distribution as _, Rng};
use serde::{Deserialize, Serialize};
use statrs::distribution::{ContinuousCDF, Empirical, Exp, LogNormal, Normal, Uniform};
use statrs::statistics::{Distribution, Max, Min};

use crate::utils::{prelude::*, BoxIterator, IntoBoxIter};

const fn default_offset() -> f64 {
    0.0
}

const fn default_factor() -> f64 {
    1.0
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
struct Transformation {
    #[serde(default = "default_offset")]
    offset: f64,
    #[serde(default = "default_factor")]
    factor: f64,
}

impl Transformation {
    pub fn apply(&self, point: f64) -> f64 {
        point * self.factor + self.offset
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
enum RandomVariableInner {
    Constant(f64),
    #[serde(deserialize_with = "helpers::uniform::deserialize")]
    Uniform {
        #[serde(skip_serializing)]
        dist: Uniform,
        low: f64,
        high: f64,
        #[serde(flatten)]
        trans: Transformation,
    },
    #[serde(deserialize_with = "helpers::normal::deserialize")]
    Normal {
        #[serde(skip_serializing)]
        dist: Normal,
        mean: f64,
        std_dev: f64,
        #[serde(flatten)]
        trans: Transformation,
    },
    #[serde(deserialize_with = "helpers::log_normal::deserialize")]
    LogNormal {
        #[serde(skip_serializing)]
        dist: LogNormal,
        location: f64,
        scale: f64,
        #[serde(flatten)]
        trans: Transformation,
    },
    #[serde(deserialize_with = "helpers::exp::deserialize")]
    Exp {
        #[serde(skip_serializing)]
        dist: Exp,
        /// cache the raw quantile99
        #[serde(skip_serializing)]
        raw_quantile99: f64,
        lambda: f64,
        #[serde(flatten)]
        trans: Transformation,
    },
    #[serde(deserialize_with = "helpers::empirical::deserialize")]
    Empirical {
        #[serde(skip_serializing)]
        dist: Empirical,
        samples: Vec<f64>,
        #[serde(flatten)]
        trans: Transformation,
    },
}

macro_rules! forward {
    ( $self:expr, { $dist:ident => $dist_res:expr, $constant:ident =>  $constant_res:expr $(,)? } ) => {
        forward!($self, {
            ($dist, _trans) => $dist_res,
            $constant => $constant_res,
        })
    };

    ( $self:expr, { ($dist:ident, $trans:ident) => $dist_res:expr, $constant:ident =>  $constant_res:expr $(,)? } ) => {
        match $self {
            RandomVariableInner::Constant($constant) => $constant_res,
            RandomVariableInner::Uniform { dist: $dist, trans: $trans, .. } => $dist_res,
            RandomVariableInner::Normal { dist: $dist, trans: $trans, .. } => $dist_res,
            RandomVariableInner::LogNormal { dist: $dist, trans: $trans, .. } => $dist_res,
            RandomVariableInner::Exp { dist: $dist, trans: $trans, .. } => $dist_res,
            RandomVariableInner::Empirical { dist: $dist, trans: $trans, .. } => $dist_res,
        }
    };
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct RandomVariable(RandomVariableInner);

impl RandomVariable {
    pub fn constant(c: f64) -> Self {
        Self(RandomVariableInner::Constant(c))
    }

    pub fn min(&self) -> f64 {
        forward!(&self.0, {
            dist => dist.min(),
            c => *c,
        })
    }

    pub fn max(&self) -> f64 {
        forward!(&self.0, {
            dist => dist.max(),
            c => *c,
        })
    }

    pub fn mean(&self) -> f64 {
        forward!(&self.0, {
            dist => dist.mean().unwrap(),
            c => *c,
        })
    }

    pub fn quantile(&self, percentage: f64) -> f64 {
        if let RandomVariableInner::Exp {
            raw_quantile99, trans, ..
        } = &self.0
        {
            if ulps_eq!(percentage, 0.99) {
                return trans.apply(*raw_quantile99);
            }
        }

        forward!(&self.0, {
            (dist, trans) => {
                let raw = dist.inverse_cdf(percentage);
                dbg!(raw);
                trans.apply(raw)
            },
            c => *c,
        })
    }

    #[allow(clippy::clone_on_copy)]
    pub fn sample_iter<'a, W, R>(&self, rng: R) -> Result<BoxIterator<'a, Observation<W>>>
    where
        W: Copy + Deref<Target = f64>,
        f64: Into<W>,
        R: Rng + 'a,
    {
        let rv = self.clone();
        let it: BoxIterator<_> = forward!(&self.0, {
            (dist, trans) => {
                let trans = *trans;
                dist
                    .clone()
                    .sample_iter(rng)
                    .map(move |v| trans.apply(v))
                    .map(move |v| Observation::new(v, rv.clone()))
                    .into_boxed()
            },
            c => std::iter::repeat(*c)
                .map(move |v| Observation::new(v, rv.clone()))
                .into_boxed(),
        });
        Ok(it)
    }
}

/// One observation of the random variable
/// `W` is the wrapper type for `f64`, and f64 should implement `Into<W>`
#[derive(Debug, Clone)]
pub struct Observation<W> {
    value: W,
    dist: RandomVariable,
}

impl<W> Observation<W>
where
    f64: Into<W>,
{
    pub fn new(v: f64, dist: RandomVariable) -> Self {
        Self { value: v.into(), dist }
    }
}

impl<W> Observation<W>
where
    W: Copy,
{
    pub fn value(&self) -> W {
        self.value
    }
}

impl<W> Observation<W> {
    pub fn dist(&self) -> &RandomVariable {
        &self.dist
    }
}

impl<W> Observation<W>
where
    f64: Into<W>,
{
    pub fn quantile(&self, per: f64) -> W {
        self.dist.quantile(per).into()
    }

    pub fn min(&self) -> W {
        self.dist.min().into()
    }

    pub fn max(&self) -> W {
        self.dist.max().into()
    }

    pub fn mean(&self) -> W {
        self.dist.mean().into()
    }
}

impl<W> PartialEq for Observation<W>
where
    W: PartialEq + Deref<Target = f64>,
{
    fn eq(&self, other: &Self) -> bool {
        self.value.eq(&other.value)
    }
}

mod helpers {
    use super::*;
    use serde::de;
    use serde::Deserializer;

    pub(super) mod uniform {
        use super::*;

        pub(in super::super) fn deserialize<'de, D>(
            deserializer: D,
        ) -> Result<(Uniform, f64, f64, Transformation), D::Error>
        where
            D: Deserializer<'de>,
        {
            #[derive(Deserialize)]
            struct Cfg {
                low: f64,
                high: f64,
                #[serde(flatten)]
                trans: Transformation,
            }

            let Cfg { low, high, trans } = Cfg::deserialize(deserializer)?;
            let dist = Uniform::new(low, high).map_err(de::Error::custom)?;
            Ok((dist, low, high, trans))
        }
    }

    pub(super) mod normal {
        use super::*;

        pub(in super::super) fn deserialize<'de, D>(
            deserializer: D,
        ) -> Result<(Normal, f64, f64, Transformation), D::Error>
        where
            D: Deserializer<'de>,
        {
            #[derive(Deserialize)]
            struct Cfg {
                mean: f64,
                std_dev: f64,
                #[serde(flatten)]
                trans: Transformation,
            }

            let Cfg { mean, std_dev, trans } = Cfg::deserialize(deserializer)?;
            let dist = Normal::new(mean, std_dev).map_err(de::Error::custom)?;
            Ok((dist, mean, std_dev, trans))
        }
    }

    pub(super) mod log_normal {
        use super::*;

        pub(in super::super) fn deserialize<'de, D>(
            deserializer: D,
        ) -> Result<(LogNormal, f64, f64, Transformation), D::Error>
        where
            D: Deserializer<'de>,
        {
            #[derive(Deserialize)]
            struct Cfg {
                location: f64,
                scale: f64,
                #[serde(flatten)]
                trans: Transformation,
            }

            let Cfg { location, scale, trans } = Cfg::deserialize(deserializer)?;
            let dist = LogNormal::new(location, scale).map_err(de::Error::custom)?;
            Ok((dist, location, scale, trans))
        }
    }

    pub(super) mod exp {
        use super::*;

        pub(in super::super) fn deserialize<'de, D>(
            deserializer: D,
        ) -> Result<(Exp, f64, f64, Transformation), D::Error>
        where
            D: Deserializer<'de>,
        {
            #[derive(Deserialize)]
            struct Cfg {
                lambda: f64,
                #[serde(flatten)]
                trans: Transformation,
            }

            let Cfg { lambda, trans } = Cfg::deserialize(deserializer)?;
            let dist = Exp::new(lambda).map_err(de::Error::custom)?;
            let quantile99 = dist.inverse_cdf(0.99);
            Ok((dist, quantile99, lambda, trans))
        }
    }

    pub(super) mod empirical {
        use super::*;

        pub(in super::super) fn deserialize<'de, D>(
            deserializer: D,
        ) -> Result<(Empirical, Vec<f64>, Transformation), D::Error>
        where
            D: Deserializer<'de>,
        {
            #[derive(Deserialize)]
            struct Cfg {
                samples: Vec<f64>,
                #[serde(flatten)]
                trans: Transformation,
            }

            let Cfg { samples, trans } = Cfg::deserialize(deserializer)?;
            let dist = Empirical::from_vec(samples.clone());
            Ok((dist, samples, trans))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transformation_works() {
        let lambda = 1.5;
        let dist = Exp::new(lambda).unwrap();
        let trans = Transformation {
            offset: 10.0,
            factor: 100.0,
        };
        let var = RandomVariable(RandomVariableInner::Exp {
            dist,
            lambda,
            trans,
            raw_quantile99: dist.inverse_cdf(0.99),
        });

        assert_eq!(var.quantile(0.99), trans.apply(dist.inverse_cdf(0.99)));
        assert_eq!(var.quantile(0.8), trans.apply(dist.inverse_cdf(0.8)));
    }
}
