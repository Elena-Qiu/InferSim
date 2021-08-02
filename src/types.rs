use std::fmt;
use std::ops::{Add, AddAssign, Mul, Sub, SubAssign};

use educe::Educe;
use parse_display::Display;
use serde::{Deserialize, Serialize};

use crate::randvars::Observation;
use crate::utils::prelude::*;

/// A time point in simulation
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, Educe, Display)]
#[educe(Deref, DerefMut, PartialEq, Eq, PartialOrd, Ord)]
#[display("{0:.2}")]
pub struct Time(
    #[educe(PartialEq(method = "utils::float::total_eq"))]
    #[educe(PartialOrd(method = "utils::float::total_cmp"))]
    #[educe(Ord(method = "f64::total_cmp"))]
    pub f64,
);

impl From<f64> for Time {
    fn from(v: f64) -> Self {
        Self(v)
    }
}

/// A duration of time in simulation
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, Educe, Display)]
#[educe(Deref, DerefMut, PartialEq, Eq, PartialOrd, Ord)]
#[display("{0:.2}")]
pub struct Duration(
    #[educe(PartialEq(method = "utils::float::total_eq"))]
    #[educe(PartialOrd(method = "utils::float::total_cmp"))]
    #[educe(Ord(method = "f64::total_cmp"))]
    pub f64,
);

impl Duration {
    pub fn is_empty(&self) -> bool {
        self.0 == 0.0
    }
}

impl From<f64> for Duration {
    fn from(v: f64) -> Self {
        Self(v)
    }
}

impl Mul<f64> for Duration {
    type Output = Duration;

    fn mul(self, rhs: f64) -> Self::Output {
        Self(self.0 * rhs)
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

/// close-open interval. [lb, ub)
#[derive(Debug, Clone, Copy, Default, Ord, PartialOrd, Eq, PartialEq, Display)]
#[display("{0}+{1}")]
pub struct TimeInterval(pub Time, pub Duration);

impl TimeInterval {
    pub fn size(&self) -> Duration {
        self.1
    }

    pub fn is_empty(&self) -> bool {
        self.1.is_empty()
    }

    pub fn lb(&self) -> Time {
        self.0
    }

    pub fn ub(&self) -> Time {
        self.0 + self.1
    }

    // TODO: remove the allow after the clippy PR #7266 hits release, which is likely to be 1.52.2
    // the grouping of the op is correct, but the clippy lint gives a false positive
    #[allow(clippy::suspicious_operation_groupings)]
    pub fn is_disjoint(&self, other: &TimeInterval) -> bool {
        self.is_empty() || other.is_empty() || self.lb() >= other.ub() || other.lb() >= self.ub()
    }

    pub fn overlap(&self, other: &TimeInterval) -> bool {
        !self.is_disjoint(other)
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
    pub fn missed_deadline(&self, now: impl Into<Time>) -> bool {
        let now = now.into();
        self.deadline.map(|d| d >= now).unwrap_or(false)
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
    pub interval: TimeInterval,
    pub done: Vec<Job>,
    pub past_due: Vec<Job>,
}

impl fmt::Display for Batch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Batch {{ done.len: {}, past_due.len: {} }}",
            self.done.len(),
            self.past_due.len()
        )
    }
}

impl Batch {
    /// the batch processing time is the max of all jobs in the batch
    pub fn latency(&self) -> Duration {
        self.interval.1
    }
    /// when did the batch started
    pub fn started(&self) -> Time {
        self.interval.0
    }
}
