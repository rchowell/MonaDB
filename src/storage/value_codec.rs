//! Value encoding for the `data` DB.
//!
//! ```text
//! [ tag : u8 ] [ body : variable ]
//!
//! tag = 0x00 → live row;       body = serde_json::to_vec(&value)
//! tag = 0x01..0xFF → reserved  (Appendix A: tombstones, large-blob pointers, etc.)
//! ```
//!
//! The tag byte is a forward-compatibility hook — future variants can land additively
//! without disturbing existing data.

use serde_json::Value as JsonValue;

use crate::error::Error;
use crate::value::Value;
use crate::Result;

pub const TAG_LIVE: u8 = 0x00;

pub fn encode(value: &Value) -> Vec<u8> {
    let body = value.to_vec();
    let mut out = Vec::with_capacity(1 + body.len());
    out.push(TAG_LIVE);
    out.extend_from_slice(&body);
    out
}

pub fn decode(bytes: &[u8]) -> Result<Value> {
    let Some((&tag, body)) = bytes.split_first() else {
        return Err(Error::InternalError(
            "storage: empty value record".to_string(),
        ));
    };
    match tag {
        TAG_LIVE => {
            let json: JsonValue = serde_json::from_slice(body)?;
            Ok(Value::new(json))
        }
        other => Err(Error::InternalError(format!(
            "storage: unknown value tag 0x{other:02x} (reserved for future use)"
        ))),
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn round_trip_number() {
        let v = Value::number(42.0);
        let bytes = encode(&v);
        assert_eq!(bytes[0], TAG_LIVE);
        let back = decode(&bytes).unwrap();
        assert_eq!(v, back);
    }

    #[test]
    fn round_trip_string() {
        let v = Value::string("hello".to_string());
        let bytes = encode(&v);
        let back = decode(&bytes).unwrap();
        assert_eq!(v, back);
    }

    #[test]
    fn unknown_tag_errors() {
        let err = decode(&[0x42]).unwrap_err();
        match err {
            Error::InternalError(msg) => assert!(msg.contains("0x42")),
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn empty_input_errors() {
        let err = decode(&[]).unwrap_err();
        assert!(matches!(err, Error::InternalError(_)));
    }
}
