pub use unique::UniqueExt;

mod unique {
    use std::{cmp::Eq, collections::HashSet, hash::Hash};

    pub struct Unique<I: Iterator> {
        iter: I,
        seen: HashSet<I::Item>,
    }

    impl<I: Iterator> Iterator for Unique<I>
    where
        I::Item: Eq + Hash + Clone,
    {
        type Item = I::Item;

        fn next(&mut self) -> Option<Self::Item> {
            while let Some(next) = self.iter.next() {
                if !self.seen.contains(&next) {
                    // TODO: may be able to remove the clone with a hash?
                    self.seen.insert(next.clone());
                    return Some(next);
                }
            }

            None
        }
    }

    pub trait UniqueExt: Iterator {
        fn unique(self) -> Unique<Self>
        where
            Self::Item: Eq + Hash + Clone,
            Self: Sized,
        {
            Unique {
                iter: self,
                seen: HashSet::new(),
            }
        }
    }

    impl<I: Iterator> UniqueExt for I {}
}
