use rand::distributions::Distribution;
use rand::Rng;

use crate::Job;

pub(crate) fn one_batch(
    rng: impl Rng,
    delay: f64,
    n_jobs: usize,
    job_distribution: impl Distribution<f64>,
) -> impl IntoIterator<Item = (f64, impl IntoIterator<Item = Job>)> {
    let jobs: Vec<_> = rng.sample_iter(job_distribution).map(Job::new).take(n_jobs).collect();
    std::iter::once((delay, jobs))
}
