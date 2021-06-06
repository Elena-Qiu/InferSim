use std::cmp::Ordering;
use std::fmt;
use std::ops::{Add, AddAssign, Sub, SubAssign};

use derive_more::{Deref, DerefMut, Display, From};
use serde::{Deserialize, Serialize};

use crate::randvars::Observation;

/// A time point in simulation
#[derive(Debug, Clone, Copy, From, Display, Deref, DerefMut, Serialize, Deserialize)]
pub struct Time(pub f64);

impl PartialEq for Time {
    fn eq(&self, other: &Self) -> bool {
        self.0.total_cmp(&other.0).is_eq()
    }
}

impl Eq for Time {}

impl PartialOrd for Time {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.0.total_cmp(&other.0))
    }
}

impl Ord for Time {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).unwrap()
    }
}

/// A duration of time in simulation
#[derive(Debug, Clone, Copy, From, Display, Deref, DerefMut, Serialize, Deserialize)]
pub struct Duration(pub f64);

impl PartialEq for Duration {
    fn eq(&self, other: &Self) -> bool {
        self.0.total_cmp(&other.0).is_eq()
    }
}

impl Eq for Duration {}

impl PartialOrd for Duration {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.0.total_cmp(&other.0))
    }
}

impl Ord for Duration {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).unwrap()
    }
}

impl Add<Duration> for Time {
    type Output = Time;

    fn add(self, rhs: Duration) -> Self::Output {
        Time(self.0 + rhs.0)
    }
}

impl AddAssign<Duration> for Time {
    fn add_assign(&mut self, rhs: Duration) {
        self.0 += rhs.0;
    }
}

impl Sub<Duration> for Time {
    type Output = Time;

    fn sub(self, rhs: Duration) -> Self::Output {
        Time(self.0 - rhs.0)
    }
}

impl SubAssign<Duration> for Time {
    fn sub_assign(&mut self, rhs: Duration) {
        self.0 -= rhs.0;
    }
}

impl Sub for Time {
    type Output = Duration;

    fn sub(self, rhs: Self) -> Self::Output {
        Duration(self.0 - rhs.0)
    }
}

/// Incoming job, not yet accepted by the system
#[derive(Debug, Clone, PartialEq)]
pub struct IncomingJob {
    /// Job ID
    pub id: usize,
    /// Inference length
    pub length: Observation<Duration>,
    /// time budget
    pub budget: Option<Duration>,
}

impl fmt::Display for IncomingJob {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.budget {
            Some(b) => write!(f, "IncomingJob({}, {:.2}, {:.2})", self.id, self.length.value(), b),
            None => write!(f, "IncomingJob({}, {:.2}, None)", self.id, self.length.value()),
        }
    }
}

impl IncomingJob {
    pub fn into_job(self, admitted: Time) -> Job {
        Job {
            id: self.id,
            admitted,
            length: self.length,
            deadline: self.budget.map(|b| admitted + b),
        }
    }
}

/// A job admitted in the system
#[derive(Debug, Clone, PartialEq)]
pub struct Job {
    pub id: usize,
    pub admitted: Time,
    pub length: Observation<Duration>,
    /// deadline, absolute
    pub deadline: Option<Time>,
}

impl Job {
    pub fn missed_deadline(&self, time: impl Into<Time>) -> bool {
        let time = time.into();
        self.deadline.map(|d| d > time).unwrap_or(false)
    }
}

impl fmt::Display for Job {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.deadline {
            Some(d) => write!(
                f,
                "Job({}, @{:.2}<{:.2}<{:.2})",
                self.id,
                self.admitted,
                self.length.value(),
                d
            ),
            None => write!(
                f,
                "Job({}, @{:.2}<{:.2}<None)",
                self.id,
                self.admitted,
                self.length.value()
            ),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Batch {
    pub id: usize,
    pub jobs: Vec<Job>,
    pub started: Time,
}

impl fmt::Display for Batch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Batch {{ jobs.len: {} }}", self.jobs.len())
    }
}

impl Batch {
    /// the batch processing time is the max of all jobs in the batch
    pub fn latency(&self) -> Duration {
        self.jobs
            .iter()
            .map(|j| j.length.value())
            .reduce(|a, b| if a < b { b } else { a })
            .expect("Batch can not be empty")
    }
}
