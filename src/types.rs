use std::fmt;
use std::ops::{Add, AddAssign, Deref, Sub};

use derive_more::Display;
use serde::{Deserialize, Serialize};

/// A time point in simulation
#[derive(Debug, Clone, Copy, PartialOrd, PartialEq, Display, Serialize, Deserialize)]
pub struct Time(pub f64);

/// A duration of time in simulation
#[derive(Debug, Clone, Copy, PartialOrd, PartialEq, Display, Serialize, Deserialize)]
pub struct Duration(pub f64);

impl Deref for Duration {
    type Target = f64;

    fn deref(&self) -> &Self::Target {
        &self.0
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
    pub length: Duration,
    /// time budget
    pub budget: Option<Duration>,
}

impl fmt::Display for IncomingJob {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.budget {
            Some(b) => write!(f, "IncomingJob({}, {:.2}, {:.2})", self.id, self.length, b),
            None => write!(f, "IncomingJob({}, {:.2}, None)", self.id, self.length),
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
    pub length: Duration,
    /// deadline, absolute
    pub deadline: Option<Time>,
}

impl Job {
    pub fn missed_deadline(&self, time: Time) -> bool {
        self.deadline.map(|d| d > time).unwrap_or(false)
    }
}

impl fmt::Display for Job {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.deadline {
            Some(d) => write!(f, "Job({}, @{:.2}<{:.2}<{:.2})", self.id, self.admitted, self.length, d),
            None => write!(f, "Job({}, @{:.2}<{:.2}<None)", self.id, self.admitted, self.length),
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
            .map(|j| j.length)
            .reduce(|a, b| if a < b { b } else { a })
            .expect("Batch can not be empty")
    }
}
