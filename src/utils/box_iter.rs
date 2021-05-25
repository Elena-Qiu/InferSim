pub trait IntoBoxIter<'a>: Iterator {
    fn into_boxed(self) -> Box<dyn Iterator<Item = Self::Item> + 'a>;
}

impl<'a, T> IntoBoxIter<'a> for T
where
    T: 'a + Iterator,
{
    fn into_boxed(self) -> Box<dyn Iterator<Item = Self::Item> + 'a> {
        Box::new(self)
    }
}

pub type BoxIterator<'a, T> = Box<dyn Iterator<Item = T> + 'a>;
