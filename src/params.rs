//! Query parameter bindings and conversions into engine [`Params`].
//!
//! [`IntoParams`] lets callers pass tuples, slices, and maps wherever a
//! statement expects bound values — the same ergonomics as rusqlite/duckdb.

use std::collections::HashMap;

use crate::value::Value;

/// Parameter bindings supplied to a query alongside its SQL text.
///
/// `?` and `$N` both draw from the positional list, indexed 1-based — the first
/// `?` and `$1` both resolve to `positional[0]`. `$name` resolves against the
/// named map. The binder substitutes each placeholder with its bound literal
/// before compilation, so a query may freely mix both kinds.
#[derive(Debug, Clone, Default)]
pub struct Params {
    positional: Vec<Value>,
    named: HashMap<String, Value>,
}

impl Params {
    /// Returns an empty parameter set.
    #[inline]
    #[must_use]
    pub fn none() -> Params {
        Params::default()
    }

    /// Builds a parameter set from a positional list (`?`, `$N`).
    #[inline]
    #[must_use]
    pub fn positional(values: Vec<Value>) -> Params {
        Params {
            positional: values,
            named: HashMap::new(),
        }
    }

    /// Builds a parameter set from a named map (`$name`).
    #[inline]
    #[must_use]
    pub fn named(named: HashMap<String, Value>) -> Params {
        Params {
            positional: Vec::new(),
            named,
        }
    }

    /// Looks up a 1-based positional/numbered parameter (`?` or `$N`).
    #[inline]
    #[must_use]
    pub fn get_numbered(&self, n: u32) -> Option<&Value> {
        if n == 0 {
            return None;
        }
        self.positional.get((n - 1) as usize)
    }

    /// Looks up a named parameter (`$name`).
    #[inline]
    #[must_use]
    pub fn get_named(&self, name: &str) -> Option<&Value> {
        self.named.get(name)
    }
}

/// Converts caller-supplied parameter values into engine [`Params`].
pub trait IntoParams {
    /// Builds a [`Params`] value from `self`.
    fn into_params(self) -> Params;
}

impl IntoParams for () {
    fn into_params(self) -> Params {
        Params::none()
    }
}

impl IntoParams for Params {
    fn into_params(self) -> Params {
        self
    }
}

impl IntoParams for &Params {
    fn into_params(self) -> Params {
        self.clone()
    }
}

impl IntoParams for Vec<Value> {
    fn into_params(self) -> Params {
        Params::positional(self)
    }
}

impl IntoParams for &[Value] {
    fn into_params(self) -> Params {
        Params::positional(self.to_vec())
    }
}

impl<T: Into<Value>, const N: usize> IntoParams for [T; N] {
    fn into_params(self) -> Params {
        Params::positional(self.into_iter().map(Into::into).collect())
    }
}

impl IntoParams for HashMap<String, Value> {
    fn into_params(self) -> Params {
        Params::named(self)
    }
}

impl IntoParams for &HashMap<String, Value> {
    fn into_params(self) -> Params {
        Params::named(self.clone())
    }
}

macro_rules! impl_into_params_tuple {
    ($($idx:tt $T:ident),+) => {
        impl<$($T: Into<Value>),+> IntoParams for ($($T,)+) {
            fn into_params(self) -> Params {
                Params::positional(vec![$(self.$idx.into()),+])
            }
        }
    };
}

impl_into_params_tuple!(0 A);
impl_into_params_tuple!(0 A, 1 B);
impl_into_params_tuple!(0 A, 1 B, 2 C);
impl_into_params_tuple!(0 A, 1 B, 2 C, 3 D);
impl_into_params_tuple!(0 A, 1 B, 2 C, 3 D, 4 E);
impl_into_params_tuple!(0 A, 1 B, 2 C, 3 D, 4 E, 5 F);
impl_into_params_tuple!(0 A, 1 B, 2 C, 3 D, 4 E, 5 F, 6 G);
impl_into_params_tuple!(0 A, 1 B, 2 C, 3 D, 4 E, 5 F, 6 G, 7 H);

/// Builds positional [`Params`] from a heterogeneous list of values.
///
/// Each argument is converted with [`Value::from`] / [`Into`] as appropriate.
#[macro_export]
macro_rules! params {
    () => {
        $crate::Params::none()
    };
    ($($v:expr),+ $(,)?) => {
        $crate::Params::positional(vec![$(::core::convert::Into::into($v)),+])
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unit_is_empty() {
        let p = <() as IntoParams>::into_params(());
        assert!(p.get_numbered(1).is_none());
    }

    #[test]
    fn tuple_positional() {
        let p = (1i64, "x").into_params();
        assert_eq!(p.get_numbered(1), Some(&Value::int(1)));
        assert_eq!(
            p.get_numbered(2),
            Some(&Value::String(std::rc::Rc::from("x")))
        );
    }

    #[test]
    fn macro_builds_positional() {
        let p = crate::params![1i64, 2i64];
        assert_eq!(p.get_numbered(1), Some(&Value::int(1)));
        assert_eq!(p.get_numbered(2), Some(&Value::int(2)));
    }
}
