use rand::seq::SliceRandom;
use rand::Rng;
use std::cmp::min;
use std::collections::VecDeque;

pub trait Batcher {
    type Item;
    fn batch_pop_front(&mut self, size: usize) -> Vec<Self::Item>;
    fn batch_pop_front_random<R>(&mut self, rng: &mut R, size: usize) -> Vec<Self::Item>
    where
        R: Rng + ?Sized;
}

impl<T> Batcher for VecDeque<T> {
    type Item = T;

    fn batch_pop_front(&mut self, size: usize) -> Vec<Self::Item> {
        self.drain(..min(self.len(), size)).collect()
    }

    fn batch_pop_front_random<R>(&mut self, rng: &mut R, size: usize) -> Vec<Self::Item>
    where
        R: Rng + ?Sized,
    {
        let batch_size = min(self.len(), size);
        if batch_size == 0 {
            vec![]
        } else {
            self.make_contiguous()
                .partial_shuffle(rng, batch_size);
            self.batch_pop_front(batch_size)
        }
    }
}

impl<T> Batcher for Vec<T> {
    type Item = T;

    fn batch_pop_front(&mut self, size: usize) -> Vec<Self::Item> {
        self.drain(..min(self.len(), size)).collect()
    }

    fn batch_pop_front_random<R>(&mut self, rng: &mut R, size: usize) -> Vec<Self::Item>
    where
        R: Rng + ?Sized,
    {
        let batch_size = min(self.len(), size);
        if batch_size == 0 {
            vec![]
        } else {
            self.partial_shuffle(rng, batch_size);
            self.batch_pop_front(batch_size)
        }
    }
}
