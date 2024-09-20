// this is copied from itertools under the following license
//
// Copyright (c) 2015
//
// Permission is hereby granted, free of charge, to any
// person obtaining a copy of this software and associated
// documentation files (the "Software"), to deal in the
// Software without restriction, including without
// limitation the rights to use, copy, modify, merge,
// publish, distribute, sublicense, and/or sell copies of
// the Software, and to permit persons to whom the Software
// is furnished to do so, subject to the following
// conditions:
//
// The above copyright notice and this permission notice
// shall be included in all copies or substantial portions
// of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF
// ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED
// TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A
// PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT
// SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY
// CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION
// OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR
// IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
// DEALINGS IN THE SOFTWARE.

use std::iter::{Fuse, FusedIterator, Peekable};

pub(crate) struct WithPosition<I>
where
    I: Iterator,
{
    handled_first: bool,
    peekable: Peekable<Fuse<I>>,
}

impl<I> WithPosition<I>
where
    I: Iterator,
{
    pub(crate) fn new(iter: impl IntoIterator<IntoIter = I>) -> WithPosition<I> {
        WithPosition {
            handled_first: false,
            peekable: iter.into_iter().fuse().peekable(),
        }
    }
}

impl<I> Clone for WithPosition<I>
where
    I: Clone + Iterator,
    I::Item: Clone,
{
    fn clone(&self) -> Self {
        Self {
            handled_first: self.handled_first,
            peekable: self.peekable.clone(),
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub(crate) enum Position<T> {
    First(T),
    Middle(T),
    Last(T),
    Only(T),
}

impl<T> Position<T> {
    pub(crate) fn into_inner(self) -> T {
        match self {
            Position::First(x) | Position::Middle(x) | Position::Last(x) | Position::Only(x) => x,
        }
    }
}

impl<I: Iterator> Iterator for WithPosition<I> {
    type Item = Position<I::Item>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.peekable.next() {
            Some(item) => {
                if !self.handled_first {
                    // Haven't seen the first item yet, and there is one to give.
                    self.handled_first = true;
                    // Peek to see if this is also the last item,
                    // in which case tag it as `Only`.
                    match self.peekable.peek() {
                        Some(_) => Some(Position::First(item)),
                        None => Some(Position::Only(item)),
                    }
                } else {
                    // Have seen the first item, and there's something left.
                    // Peek to see if this is the last item.
                    match self.peekable.peek() {
                        Some(_) => Some(Position::Middle(item)),
                        None => Some(Position::Last(item)),
                    }
                }
            }
            // Iterator is finished.
            None => None,
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.peekable.size_hint()
    }
}

impl<I> ExactSizeIterator for WithPosition<I> where I: ExactSizeIterator {}

impl<I: Iterator> FusedIterator for WithPosition<I> {}
