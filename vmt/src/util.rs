use std::borrow::Cow;

pub(crate) fn apply<T: Clone>(a: Option<T>, b: &Option<T>) -> Option<T> {
    if let Some(b) = b {
        Some(b.clone())
    } else {
        a
    }
}

// TODO: it might be more efficient to just store them as `Cow<'_, str>`s without
// converting to lowercase, and then just have accessors that check for equality to lowercase
// That would be less efficient than normal hashmap access, but it would avoid the allocation
// and the hashmaps are usually quite small?
pub(crate) fn to_lowercase_cow_str(text: &str) -> Cow<'_, str> {
    if text.chars().any(|c| c.is_ascii_uppercase()) {
        Cow::Owned(text.to_ascii_lowercase())
    } else {
        Cow::Borrowed(text)
    }
}

pub(crate) fn to_lowercase_cow(text: &[u8]) -> Cow<'_, [u8]> {
    if text.iter().any(|c| c.is_ascii_uppercase()) {
        Cow::Owned(text.to_ascii_lowercase())
    } else {
        Cow::Borrowed(text)
    }
}

pub(crate) struct StopOnErr<I, T, E>
where
    I: Iterator<Item = Result<T, E>>,
{
    inner: I,
    stopped: bool,
}

impl<I, T, E> StopOnErr<I, T, E>
where
    I: Iterator<Item = Result<T, E>>,
{
    pub(crate) fn new(iter: I) -> Self {
        StopOnErr {
            inner: iter,
            stopped: false,
        }
    }
}

impl<I, T, E> Iterator for StopOnErr<I, T, E>
where
    I: Iterator<Item = Result<T, E>>,
{
    type Item = Result<T, E>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.stopped {
            None
        } else {
            match self.inner.next() {
                Some(Ok(item)) => Some(Ok(item)),
                Some(Err(e)) => {
                    self.stopped = true;
                    Some(Err(e))
                }
                None => None,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stop_on_first_err() {
        let results = vec![Ok(1), Ok(2), Err("first error"), Ok(3), Err("second error")];
        let mut iter = StopOnErr::new(results.into_iter());

        assert_eq!(iter.next(), Some(Ok(1)));
        assert_eq!(iter.next(), Some(Ok(2)));
        assert_eq!(iter.next(), Some(Err("first error")));
        assert_eq!(iter.next(), None); // Iteration should stop after the first error
    }

    #[test]
    fn test_no_err() {
        let results: Vec<Result<i32, ()>> = vec![Ok(1), Ok(2), Ok(3)];
        let mut iter = StopOnErr::new(results.into_iter());

        assert_eq!(iter.next(), Some(Ok(1)));
        assert_eq!(iter.next(), Some(Ok(2)));
        assert_eq!(iter.next(), Some(Ok(3)));
        assert_eq!(iter.next(), None); // Iteration ends normally as there are no errors
    }
}
