use std::cmp::Ordering;

pub fn total_eq(a: &f64, b: &f64) -> bool {
    a.total_cmp(b).is_eq()
}

pub fn total_cmp(a: &f64, b: &f64) -> Option<Ordering> {
    Some(a.total_cmp(b))
}
