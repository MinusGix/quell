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
pub(crate) fn to_lowercase_cow(text: &str) -> Cow<'_, str> {
    if text.chars().any(|c| c.is_ascii_uppercase()) {
        Cow::Owned(text.to_ascii_lowercase())
    } else {
        Cow::Borrowed(text)
    }
}
