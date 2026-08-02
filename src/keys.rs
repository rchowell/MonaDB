//! Order-preserving key codec: Python keys <-> lexicographically comparable bytes.
//!
//!   tag   payload
//!   0x01  8 bytes   int    i64 big-endian, sign bit flipped
//!   0x02  var       str    UTF-8, 0x00-terminated, 0x00 escaped as 0x00 0xFF
//!   0x03  var       bytes  raw, same termination and escaping
//!
//!   tuple  = components concatenated in order
//!   scalar = a 1-component tuple
//!
//! redb compares `&[u8]` keys lexicographically, so encoded order *is* iteration
//! order. The tag byte makes the encoding self-describing, letting a stored key
//! decode back to the exact Python value that was written.

use pyo3::exceptions::{PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyBool, PyBytes, PyInt, PyString, PyTuple};

const TAG_INT: u8 = 0x01;
const TAG_STR: u8 = 0x02;
const TAG_BYTES: u8 = 0x03;

/// A single key component.
///
/// Variant order matches tag order, so the derived `Ord` on `KeyPart` — and on
/// `Vec<KeyPart>` — is the model that [`encode`] must agree with byte for byte.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum KeyPart {
    Int(i64),
    Str(String),
    Bytes(Vec<u8>),
}

/// Encodes a key (a tuple of components) to order-preserving bytes.
pub fn encode(parts: &[KeyPart]) -> Vec<u8> {
    let mut out = Vec::with_capacity(parts.len() * 10);
    for p in parts {
        match p {
            KeyPart::Int(i) => {
                out.push(TAG_INT);
                out.extend_from_slice(&(i.cast_unsigned() ^ (1u64 << 63)).to_be_bytes());
            }
            KeyPart::Str(s) => {
                out.push(TAG_STR);
                escape_into(s.as_bytes(), &mut out);
            }
            KeyPart::Bytes(b) => {
                out.push(TAG_BYTES);
                escape_into(b, &mut out);
            }
        }
    }
    out
}

/// Appends `data` with `0x00` -> `0x00 0xFF` escaping, then the `0x00` terminator.
///
/// Escaping keeps the terminator unambiguous while preserving order: a real
/// `0x00` byte becomes `0x00 0xFF`, which sorts after the bare terminator, so a
/// shorter component still sorts before any component extending it.
fn escape_into(data: &[u8], out: &mut Vec<u8>) {
    for &b in data {
        out.push(b);
        if b == 0x00 {
            out.push(0xFF);
        }
    }
    out.push(0x00);
}

/// Decodes key bytes back to components — the exact inverse of [`encode`].
pub fn decode(bytes: &[u8]) -> Result<Vec<KeyPart>, String> {
    let mut parts = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let tag = bytes[i];
        i += 1;
        match tag {
            TAG_INT => {
                let end = i + 8;
                let raw: [u8; 8] = bytes
                    .get(i..end)
                    .ok_or("truncated int key")?
                    .try_into()
                    .map_err(|_| "truncated int key")?;
                parts.push(KeyPart::Int(
                    (u64::from_be_bytes(raw) ^ (1u64 << 63)).cast_signed(),
                ));
                i = end;
            }
            TAG_STR | TAG_BYTES => {
                let (data, next) = unescape_from(bytes, i)?;
                parts.push(if tag == TAG_STR {
                    KeyPart::Str(String::from_utf8(data).map_err(|e| e.to_string())?)
                } else {
                    KeyPart::Bytes(data)
                });
                i = next;
            }
            t => return Err(format!("bad key tag {t:#x}")),
        }
    }
    Ok(parts)
}

/// Reads an escaped, `0x00`-terminated run starting at `i`.
///
/// Returns the payload and the index just past the terminator.
fn unescape_from(bytes: &[u8], mut i: usize) -> Result<(Vec<u8>, usize), String> {
    let mut data = Vec::new();
    while i < bytes.len() {
        match bytes[i] {
            0x00 if bytes.get(i + 1) == Some(&0xFF) => {
                data.push(0x00);
                i += 2;
            }
            0x00 => return Ok((data, i + 1)),
            b => {
                data.push(b);
                i += 1;
            }
        }
    }
    Err("unterminated key component".into())
}

/// Smallest byte string greater than every string prefixed by `prefix`.
///
/// `None` when `prefix` is all `0xFF` — there is no such bound, and the range
/// is then open-ended above.
pub fn successor(prefix: &[u8]) -> Option<Vec<u8>> {
    let mut out = prefix.to_vec();
    while let Some(last) = out.pop() {
        if last < 0xFF {
            out.push(last + 1);
            return Some(out);
        }
    }
    None
}

/// Converts a Python key — a scalar or a flat tuple — to components.
///
/// `TypeError` for float, bool, `None`, a nested tuple, or anything unlisted;
/// `ValueError` for an int outside the i64 range.
pub fn key_from_py(obj: &Bound<'_, PyAny>) -> PyResult<Vec<KeyPart>> {
    if let Ok(t) = obj.cast::<PyTuple>() {
        t.iter().map(|item| part_from_py(&item)).collect()
    } else {
        Ok(vec![part_from_py(obj)?])
    }
}

fn part_from_py(obj: &Bound<'_, PyAny>) -> PyResult<KeyPart> {
    // `bool` is checked first: it is a subclass of `int` in Python, and letting
    // it through would silently alias True with 1.
    if obj.cast::<PyBool>().is_ok() {
        return Err(PyTypeError::new_err("bool is not a valid key type"));
    }
    if let Ok(s) = obj.cast::<PyString>() {
        return Ok(KeyPart::Str(s.to_string()));
    }
    if let Ok(b) = obj.cast::<PyBytes>() {
        return Ok(KeyPart::Bytes(b.as_bytes().to_vec()));
    }
    if obj.cast::<PyInt>().is_ok() {
        let v: i64 = obj
            .extract()
            .map_err(|_| PyValueError::new_err("int key outside 64-bit range"))?;
        return Ok(KeyPart::Int(v));
    }
    Err(PyTypeError::new_err(format!(
        "invalid key type {} (expected str | int | bytes | tuple of those)",
        obj.get_type().name()?
    )))
}

/// Decodes stored key bytes to the Python value that was written: a scalar for
/// one component, a tuple otherwise.
pub fn key_to_py(py: Python<'_>, bytes: &[u8]) -> PyResult<Py<PyAny>> {
    let parts = decode(bytes).map_err(crate::error::internal)?;
    let mut objs: Vec<Py<PyAny>> = parts
        .into_iter()
        .map(|p| part_to_py(py, p))
        .collect::<PyResult<_>>()?;
    if objs.len() == 1 {
        Ok(objs.pop().expect("length checked"))
    } else {
        Ok(PyTuple::new(py, objs)?.into_any().unbind())
    }
}

fn part_to_py(py: Python<'_>, part: KeyPart) -> PyResult<Py<PyAny>> {
    Ok(match part {
        KeyPart::Int(i) => i.into_pyobject(py)?.into_any().unbind(),
        KeyPart::Str(s) => s.into_pyobject(py)?.into_any().unbind(),
        KeyPart::Bytes(b) => PyBytes::new(py, &b).into_any().unbind(),
    })
}

/// Encodes a `prefix()` argument to the raw lower bound of its range.
///
/// A `str` or `bytes` prefix encodes as tag plus escaped payload with **no
/// terminator**, so it matches every key of that type extending it. A tuple
/// encodes fully, terminators included, so the match is on whole leading
/// components. The upper bound is [`successor`] of the result.
pub fn prefix_from_py(obj: &Bound<'_, PyAny>) -> PyResult<Vec<u8>> {
    if obj.cast::<PyTuple>().is_ok() {
        return Ok(encode(&key_from_py(obj)?));
    }
    let part = part_from_py(obj)?;
    let mut out = Vec::new();
    match part {
        KeyPart::Str(s) => {
            out.push(TAG_STR);
            escape_no_term(s.as_bytes(), &mut out);
        }
        KeyPart::Bytes(b) => {
            out.push(TAG_BYTES);
            escape_no_term(&b, &mut out);
        }
        KeyPart::Int(_) => {
            return Err(PyTypeError::new_err(
                "prefix must be str, bytes, or a tuple of key components",
            ));
        }
    }
    Ok(out)
}

fn escape_no_term(data: &[u8], out: &mut Vec<u8>) {
    for &b in data {
        out.push(b);
        if b == 0x00 {
            out.push(0xFF);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn corpus() -> Vec<Vec<KeyPart>> {
        let ints = [i64::MIN, -2, -1, 0, 1, 2, 255, 256, i64::MAX];
        let strs = ["", "a", "a\u{0}b", "ab", "b", "é"];
        let bins: [&[u8]; 5] = [b"", b"\x00", b"\x00\xff", b"a", b"\xff"];
        let mut scalars = Vec::new();
        scalars.extend(ints.iter().map(|&i| KeyPart::Int(i)));
        scalars.extend(strs.iter().map(|s| KeyPart::Str((*s).into())));
        scalars.extend(bins.iter().map(|b| KeyPart::Bytes(b.to_vec())));
        let mut out: Vec<Vec<KeyPart>> = vec![vec![]];
        out.extend(scalars.iter().cloned().map(|p| vec![p]));
        for a in &scalars {
            for b in &scalars {
                out.push(vec![a.clone(), b.clone()]);
            }
        }
        out
    }

    /// `encode(a) < encode(b)` iff `a < b` — derived `Ord` on `Vec<KeyPart>` is
    /// the model. This property is what makes iteration order and every range
    /// bound correct, so it is checked over every pair in the corpus.
    #[test]
    fn encoding_preserves_order() {
        let keys = corpus();
        for a in &keys {
            for b in &keys {
                assert_eq!(
                    encode(a).cmp(&encode(b)),
                    a.cmp(b),
                    "order disagreement: {a:?} vs {b:?}"
                );
            }
        }
    }

    #[test]
    fn encoding_roundtrips() {
        for k in corpus() {
            assert_eq!(decode(&encode(&k)).unwrap(), k);
        }
    }

    #[test]
    fn successor_bounds_prefixes() {
        // Every key extending `p` sorts >= p and < successor(p).
        let p = encode(&[KeyPart::Str("ab".into())]);
        let ext = encode(&[KeyPart::Str("ab\u{ffff}z".into())]);
        let up = successor(&p[..p.len() - 1]).unwrap(); // prefix without terminator
        assert!(ext < up);
        assert_eq!(successor(&[0xff, 0xff]), None);
    }
}
