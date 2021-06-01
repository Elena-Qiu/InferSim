use itertools::Itertools;

use crate::types::{Batch, Job};
use crate::utils::prelude::*;

#[derive(Debug)]
pub struct Worker {
    total_slots: usize,
    available_slots: usize,
    batch_size: usize,
}

impl Worker {
    pub fn running(&self) -> bool {
        self.available_slots < self.total_slots
    }

    pub fn batch_start(&mut self, batch: &Vec<Job>) {
        self.available_slots -= 1;
    }

    pub fn batch_done(&mut self, batch: &Batch) {
        self.available_slots += 1;
    }

    pub fn batch_size(&self) -> usize {
        self.batch_size
    }

    pub fn available_slots(&self) -> usize {
        self.available_slots
    }
}

pub fn from_config<'a>(cfg: impl IntoIterator<Item = &'a WorkerConfig>) -> Vec<Worker> {
    cfg.into_iter()
        .map(|c| Worker {
            total_slots: c.slots,
            available_slots: c.slots,
            batch_size: c.batch_size,
        })
        .collect_vec()
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct WorkerConfig {
    slots: usize,
    batch_size: usize,
}
