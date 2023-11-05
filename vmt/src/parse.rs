use std::borrow::Cow;

use crate::{util::to_lowercase_cow, VMTError, VMTSub, VMTSubs};

pub(crate) fn parse_sub<'a>(
    b: &'a [u8],
    key: &'a [u8],
    root: &mut VMTSubs<'a>,
) -> Result<&'a [u8], VMTError> {
    let b = expect_char(b, b'{')?;
    let b = take_whitespace(b)?;

    let mut sub = VMTSubs::default();

    let mut b_cur = b;
    loop {
        let b = b_cur;

        if b.starts_with(b"}") {
            break;
        }

        if b.is_empty() {
            return Err(VMTError::Expected('}'));
        }

        let (b, key_name) = take_text(b)?;

        let b = take_whitespace(b)?;

        if b.starts_with(b"{") {
            let b = parse_sub(b, key_name, &mut sub)?;

            b_cur = b;

            continue;
        }

        let (b, val) = take_text(b)?;
        let val = std::str::from_utf8(val)?;

        let b = take_whitespace(b)?;

        let key_name = to_lowercase_cow(key_name);

        sub.0.insert(key_name, VMTSub::Val(Cow::Borrowed(val)));

        let b = take_whitespace(b)?;

        b_cur = b;
    }

    let b = b_cur;
    let b = expect_char(b, b'}')?;
    let b = take_whitespace(b)?;

    let key = to_lowercase_cow(key);
    root.0.insert(key, VMTSub::Sub(sub));

    Ok(b)
}

pub(crate) fn expect_char(bytes: &[u8], c: u8) -> Result<&[u8], VMTError> {
    if bytes.is_empty() {
        return Err(VMTError::Expected(c as char));
    }

    if bytes[0] != c {
        return Err(VMTError::Expected(c as char));
    }

    Ok(&bytes[1..])
}

pub(crate) fn take_whitespace(bytes: &[u8]) -> Result<&[u8], VMTError> {
    let end = bytes
        .iter()
        .position(|&b| !b.is_ascii_whitespace())
        .unwrap_or(bytes.len());

    Ok(&bytes[end..])
}

/// Parse a single non-whitespaced separated word
/// or a quoted string
pub(crate) fn take_text<'a>(bytes: &'a [u8]) -> Result<(&'a [u8], &'a [u8]), VMTError> {
    if bytes.starts_with(b"\"") {
        return take_str(bytes);
    }

    let end = bytes
        .iter()
        .position(|&b| b.is_ascii_whitespace())
        .unwrap_or(bytes.len());

    let (name, bytes) = bytes.split_at(end);

    Ok((bytes, name))
}

/// Parse a string like `"LightmappedGeneric"`
pub(crate) fn take_str(bytes: &[u8]) -> Result<(&[u8], &[u8]), VMTError> {
    if !bytes.starts_with(b"\"") {
        return Err(VMTError::NoStringStart);
    }

    let bytes = &bytes[1..];

    let end = bytes
        .iter()
        .position(|&b| b == b'"')
        .ok_or(VMTError::NoStringEnd)?;

    let (name, bytes) = bytes.split_at(end);

    Ok((&bytes[1..], name))
}

pub(crate) fn take_vec2(bytes: &[u8]) -> Result<(&[u8], [f32; 2]), VMTError> {
    let b = expect_char(bytes, b'[')?;
    let b = take_whitespace(b)?;
    let (b, x) = take_text(b)?;
    let b = take_whitespace(b)?;
    let (b, y) = take_text(b)?;
    let b = take_whitespace(b)?;
    let b = expect_char(b, b']')?;

    let x = std::str::from_utf8(x)?.parse()?;
    let y = std::str::from_utf8(y)?.parse()?;

    Ok((b, [x, y]))
}

/// Parse text like `[ 0.4 0.3 0.2 ]`
pub(crate) fn take_vec3(bytes: &[u8]) -> Result<(&[u8], [f32; 3]), VMTError> {
    let b = expect_char(bytes, b'[')?;
    let b = take_whitespace(b)?;
    let (b, x) = take_text(b)?;
    let b = take_whitespace(b)?;
    let (b, y) = take_text(b)?;
    let b = take_whitespace(b)?;
    let (b, z) = take_text(b)?;
    let b = take_whitespace(b)?;
    let b = expect_char(b, b']')?;

    let x = std::str::from_utf8(x)?.parse()?;
    let y = std::str::from_utf8(y)?.parse()?;
    let z = std::str::from_utf8(z)?.parse()?;

    Ok((b, [x, y, z]))
}

#[cfg(test)]
mod test {
    use crate::take_text;

    use super::take_str;

    #[test]
    fn test_take_str() {
        let bytes = b"\"LightmappedGeneric\"";
        let (bytes, name) = take_str(bytes).unwrap();
        assert_eq!(bytes, b"");
        assert_eq!(name, b"LightmappedGeneric");

        let bytes = b"\"LightmappedGeneric\" \"VertexLitGeneric\"";
        let (bytes, name) = take_str(bytes).unwrap();
        assert_eq!(bytes, b" \"VertexLitGeneric\"");
        assert_eq!(name, b"LightmappedGeneric");
        let bytes = &bytes[1..];
        let (bytes, name) = take_str(bytes).unwrap();
        assert_eq!(bytes, b"");
        assert_eq!(name, b"VertexLitGeneric");
    }

    #[test]
    fn test_take_text() {
        let bytes = b"LightmappedGeneric";
        let (bytes, name) = take_text(bytes).unwrap();
        assert_eq!(bytes, b"");
        assert_eq!(name, b"LightmappedGeneric");

        let bytes = b"LightmappedGeneric VertexLitGeneric";
        let (bytes, name) = take_text(bytes).unwrap();
        assert_eq!(bytes, b" VertexLitGeneric");
        assert_eq!(name, b"LightmappedGeneric");
        let bytes = &bytes[1..];
        let (bytes, name) = take_text(bytes).unwrap();
        assert_eq!(bytes, b"");
        assert_eq!(name, b"VertexLitGeneric");

        let bytes = b"\"LightmappedGeneric\"";
        let (bytes, name) = take_text(bytes).unwrap();
        assert_eq!(bytes, b"");
        assert_eq!(name, b"LightmappedGeneric");

        let bytes = b"\"LightmappedGeneric\" \"VertexLitGeneric\"";
        let (bytes, name) = take_text(bytes).unwrap();
        assert_eq!(bytes, b" \"VertexLitGeneric\"");
        assert_eq!(name, b"LightmappedGeneric");
        let bytes = &bytes[1..];
        let (bytes, name) = take_text(bytes).unwrap();
        assert_eq!(bytes, b"");
        assert_eq!(name, b"VertexLitGeneric");
    }
}
