use super::*;

pub fn from_config(cfg: &SchedulerConfig, rng: impl Rng + 'static) -> Result<Box<dyn Scheduler + 'static>> {
    Ok(match cfg {
        SchedulerConfig::FIFO => Box::new(FIFO),
        SchedulerConfig::Random { seed } => match seed {
            Some(seed) => {
                let rng: SipRng = Seeder::from(seed).make_rng();
                Box::new(Random { rng })
            }
            _ => Box::new(Random { rng }),
        },
        SchedulerConfig::My { percentile } => Box::new(My {
            percentile: *percentile,
        }),
    })
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub enum SchedulerConfig {
    FIFO,
    Random {
        /// Optional seed. If none, will use the same random generator as the one for incoming jobs
        seed: Option<String>,
    },
    My {
        percentile: f64,
    },
}
