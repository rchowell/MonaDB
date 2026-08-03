use bson::raw::{RawBsonRef, RawDocument};
use bson::spec::BinarySubtype;
use bson::{Binary, Bson, Document};
use pyo3::exceptions::{PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyBool, PyBytes, PyDateTime, PyDict, PyFloat, PyInt, PyList, PyString};

/// Serializes a Python mapping to BSON bytes.
pub fn doc_to_bytes(obj: &Bound<'_, PyAny>) -> PyResult<Vec<u8>> {
    let dict = obj.cast::<PyDict>().map_err(|_| {
        PyTypeError::new_err(format!(
            "document must be a mapping, got {}",
            obj.get_type()
                .name()
                .map(|n| n.to_string())
                .unwrap_or_default()
        ))
    })?;
    dict_to_bson(dict, "$")?
        .to_vec()
        .map_err(crate::error::internal)
}

fn dict_to_bson(dict: &Bound<'_, PyDict>, path: &str) -> PyResult<Document> {
    let mut doc = Document::new();
    for (k, v) in dict {
        let key: String = k
            .extract()
            .map_err(|_| PyTypeError::new_err(format!("non-string field name at {path}")))?;
        let child = format!("{path}.{key}");
        doc.insert(key, value_to_bson(&v, &child)?);
    }
    Ok(doc)
}

fn value_to_bson(v: &Bound<'_, PyAny>, path: &str) -> PyResult<Bson> {
    if v.is_none() {
        return Ok(Bson::Null);
    }
    // `bool` precedes `int`: in Python it is a subclass of one.
    if let Ok(b) = v.cast::<PyBool>() {
        return Ok(Bson::Boolean(b.is_true()));
    }
    if v.cast::<PyInt>().is_ok() {
        let i: i64 = v
            .extract()
            .map_err(|_| PyValueError::new_err(format!("int out of 64-bit range at {path}")))?;
        return Ok(if let Ok(small) = i32::try_from(i) {
            Bson::Int32(small)
        } else {
            Bson::Int64(i)
        });
    }
    if let Ok(f) = v.cast::<PyFloat>() {
        return Ok(Bson::Double(f.value()));
    }
    if let Ok(s) = v.cast::<PyString>() {
        return Ok(Bson::String(s.to_string()));
    }
    if let Ok(b) = v.cast::<PyBytes>() {
        return Ok(Bson::Binary(Binary {
            subtype: BinarySubtype::Generic,
            bytes: b.as_bytes().to_vec(),
        }));
    }
    if let Ok(dt) = v.cast::<PyDateTime>() {
        return Ok(Bson::DateTime(bson::DateTime::from_millis(utc_millis(
            dt,
        )?)));
    }
    if let Ok(d) = v.cast::<PyDict>() {
        return Ok(Bson::Document(dict_to_bson(d, path)?));
    }
    if let Ok(l) = v.cast::<PyList>() {
        let mut arr = Vec::with_capacity(l.len());
        for (i, item) in l.iter().enumerate() {
            arr.push(value_to_bson(&item, &format!("{path}[{i}]"))?);
        }
        return Ok(Bson::Array(arr));
    }
    Err(PyTypeError::new_err(format!(
        "unsupported type {} at {path}",
        v.get_type().name()?
    )))
}

/// Epoch milliseconds in UTC.
///
/// Computed by calling Python's own `.timestamp()` rather than reading struct
/// fields: under `abi3` the `PyDateTime` field accessors are unavailable. A
/// naive datetime has UTC attached first, which is exactly the documented
/// "naive datetimes are written as UTC" rule.
fn utc_millis(dt: &Bound<'_, PyDateTime>) -> PyResult<i64> {
    let py = dt.py();
    let aware = if dt.getattr("tzinfo")?.is_none() {
        let tz = py.import("datetime")?.getattr("timezone")?.getattr("utc")?;
        let kwargs = PyDict::new(py);
        kwargs.set_item("tzinfo", tz)?;
        dt.call_method("replace", (), Some(&kwargs))?
    } else {
        dt.clone().into_any()
    };
    let secs: f64 = aware.call_method0("timestamp")?.extract()?;
    #[allow(clippy::cast_possible_truncation)]
    Ok((secs * 1000.0).round() as i64)
}

/// Deserializes stored BSON bytes into a fresh `PyDict`.
pub fn bytes_to_doc(py: Python<'_>, bytes: &[u8]) -> PyResult<Py<PyDict>> {
    let raw = RawDocument::from_bytes(bytes).map_err(crate::error::internal)?;
    raw_to_dict(py, raw).map(Bound::unbind)
}

fn raw_to_dict<'py>(py: Python<'py>, raw: &RawDocument) -> PyResult<Bound<'py, PyDict>> {
    let dict = PyDict::new(py);
    for elem in raw {
        let (name, value) = elem.map_err(crate::error::internal)?;
        dict.set_item(name.as_str(), raw_to_py(py, value)?)?;
    }
    Ok(dict)
}

fn raw_to_py<'py>(py: Python<'py>, v: RawBsonRef<'_>) -> PyResult<Bound<'py, PyAny>> {
    Ok(match v {
        RawBsonRef::Null => py.None().into_bound(py),
        RawBsonRef::Boolean(b) => PyBool::new(py, b).to_owned().into_any(),
        RawBsonRef::Int32(i) => i.into_pyobject(py)?.into_any(),
        RawBsonRef::Int64(i) => i.into_pyobject(py)?.into_any(),
        RawBsonRef::Double(f) => f.into_pyobject(py)?.into_any(),
        RawBsonRef::String(s) => s.into_pyobject(py)?.into_any(),
        RawBsonRef::Binary(b) => PyBytes::new(py, b.bytes).into_any(),
        RawBsonRef::DateTime(dt) => millis_to_py(py, dt.timestamp_millis())?,
        RawBsonRef::Document(d) => raw_to_dict(py, d)?.into_any(),
        RawBsonRef::Array(a) => {
            let list = PyList::empty(py);
            for item in a {
                list.append(raw_to_py(py, item.map_err(crate::error::internal)?)?)?;
            }
            list.into_any()
        }
        other => {
            return Err(crate::error::internal(format!(
                "unexpected BSON element {other:?} in stored document"
            )));
        }
    })
}

/// Builds a tz-aware UTC datetime, via the `datetime` module for the same
/// `abi3` reason as [`utc_millis`].
fn millis_to_py(py: Python<'_>, millis: i64) -> PyResult<Bound<'_, PyAny>> {
    let dt_mod = py.import("datetime")?;
    let tz = dt_mod.getattr("timezone")?.getattr("utc")?;
    #[allow(clippy::cast_precision_loss)]
    let secs = millis as f64 / 1000.0;
    dt_mod
        .getattr("datetime")?
        .call_method1("fromtimestamp", (secs, tz))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Property: py -> bytes -> py is identity under Python `==`.
    #[test]
    fn bson_roundtrip() {
        Python::attach(|py| {
            let cases = [
                "{}",
                "{'a': 1}",
                "{'a': None, 'b': True, 'c': False}",
                "{'i32': 7, 'i64': 2**40, 'neg': -2**40, 'f': 1.5}",
                "{'s': 'héllo', 'b': b'\\x00\\xff'}",
                "{'l': [1, 'two', None, [3, {'x': 1}]]}",
                "{'n': {'deep': {'er': []}}}",
            ];
            for src in cases {
                let obj = py
                    .eval(&std::ffi::CString::new(src).unwrap(), None, None)
                    .unwrap();
                let bytes = doc_to_bytes(&obj).unwrap();
                let back = bytes_to_doc(py, &bytes).unwrap();
                assert!(back.bind(py).eq(&obj).unwrap(), "round trip changed {src}");
            }
        });
    }

    #[test]
    fn rejections() {
        Python::attach(|py| {
            let non_mapping = py.eval(c"[1, 2]", None, None).unwrap();
            assert!(doc_to_bytes(&non_mapping).is_err()); // TypeError: top level
            let bad_nested = py.eval(c"{'a': {'b': [1, {1, 2}]}}", None, None).unwrap();
            let err = doc_to_bytes(&bad_nested).unwrap_err();
            assert!(err.to_string().contains("a.b[1]"), "path missing: {err}");
            let too_big = py.eval(c"{'x': 2**63}", None, None).unwrap();
            let err = doc_to_bytes(&too_big).unwrap_err();
            assert!(err.is_instance_of::<PyValueError>(py));
        });
    }

    #[test]
    fn datetime_roundtrip_ms() {
        Python::attach(|py| {
            let ns = PyDict::new(py);
            py.run(
                c"import datetime as dt\nd = {'at': dt.datetime(2026, 8, 2, 12, 0, 0, 123456, tzinfo=dt.timezone.utc)}\nexpect = {'at': d['at'].replace(microsecond=123000)}",
                None,
                Some(&ns),
            )
            .unwrap();
            let d = ns.get_item("d").unwrap().unwrap();
            let expect = ns.get_item("expect").unwrap().unwrap();
            let back = bytes_to_doc(py, &doc_to_bytes(&d).unwrap()).unwrap();
            assert!(back.bind(py).eq(&expect).unwrap());
        });
    }

    /// Naive datetimes are written as UTC and read back tz-aware.
    #[test]
    fn naive_datetime_is_utc() {
        Python::attach(|py| {
            let ns = PyDict::new(py);
            py.run(
                c"import datetime as dt\nd = {'at': dt.datetime(2026, 8, 2, 12, 0, 0)}\nexpect = {'at': d['at'].replace(tzinfo=dt.timezone.utc)}",
                None,
                Some(&ns),
            )
            .unwrap();
            let d = ns.get_item("d").unwrap().unwrap();
            let expect = ns.get_item("expect").unwrap().unwrap();
            let back = bytes_to_doc(py, &doc_to_bytes(&d).unwrap()).unwrap();
            assert!(back.bind(py).eq(&expect).unwrap());
        });
    }
}
