use std::collections::HashSet;
use std::hash::Hash;

pub trait OrderedSequence {
    type Token;

    type Iter<'seq>: Iterator<Item = Self::Token>
    where
        Self: 'seq;

    fn iterator(&self) -> Self::Iter<'_>;
}

impl OrderedSequence for &str {
    type Token = char;

    type Iter<'seq> = std::str::Chars<'seq> where Self: 'seq;

    fn iterator(&self) -> Self::Iter<'_> {
        self.chars()
    }
}

impl OrderedSequence for &[u8] {
    type Token = u8;

    type Iter<'seq> = std::iter::Copied<std::slice::Iter<'seq, u8>> where Self: 'seq;

    fn iterator(&self) -> Self::Iter<'_> {
        self.iter().copied()
    }
}

impl OrderedSequence for char {
    type Token = char;

    type Iter<'seq> = core::iter::Once<char> where Self: 'seq;

    fn iterator(&self) -> Self::Iter<'_> {
        core::iter::once(*self)
    }
}

impl OrderedSequence for u8 {
    type Token = u8;

    type Iter<'seq> = core::iter::Once<u8> where Self: 'seq;

    fn iterator(&self) -> Self::Iter<'_> {
        core::iter::once(*self)
    }
}

pub trait Container: Default {
    type Item;

    fn push(&mut self, item: Self::Item);
}

impl<T> Container for Vec<T> {
    type Item = T;

    fn push(&mut self, item: Self::Item) {
        self.push(item);
    }
}

impl Container for String {
    type Item = char;

    fn push(&mut self, item: Self::Item) {
        self.push(item);
    }
}

impl Container for () {
    type Item = char;

    fn push(&mut self, _: Self::Item) {}
}

impl<K> Container for HashSet<K>
where
    K: Hash + Eq,
{
    type Item = K;

    fn push(&mut self, item: Self::Item) {
        self.insert(item);
    }
}
