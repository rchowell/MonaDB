#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    clippy::unnecessary_wraps
)]

use std::cmp::Ordering;
use std::rc::Rc;

use crate::error::{Error, Result};
use crate::read::{self, FileFormat, ReadOptions, WriteOptions};
use crate::value::Value;

/// How many arguments a builtin accepts, checked at compile time.
enum Arity {
    /// Exactly `n` arguments.
    Exact(usize),
    /// Between `min` and `max` arguments, inclusive.
    Range(usize, usize),
    /// `min` or more arguments (variadic).
    AtLeast(usize),
}

impl Arity {
    /// Whether `argc` satisfies this arity.
    fn ok(&self, argc: usize) -> bool {
        match self {
            Arity::Exact(n) => argc == *n,
            Arity::Range(min, max) => (*min..=*max).contains(&argc),
            Arity::AtLeast(min) => argc >= *min,
        }
    }
}

/// One entry in the standard-library registry.
struct Builtin {
    /// The name written in SQL — the call target.
    name: &'static str,
    /// Accepted argument count.
    arity: Arity,
    /// When true, a `null` argument short-circuits to a `null` result.
    strict: bool,
    /// The implementation; receives validated-arity, (if strict) non-null args.
    func: fn(&[Value]) -> Result<Value>,
}

/// The standard library: a flat list of builtins. Order defines each function's
/// registry index, which the compiler bakes into `Vop::Call`. Aliases point a
/// second name at the same `fn` (e.g. `ceiling`→`ceil`, `power`→`pow`).
#[rustfmt::skip]
static BUILTINS: &[Builtin] = &[
    //   name                   arity              strict  func
    // -- type / conditional (null-aware) ------------------------------------
    e("typeof",            Arity::Exact(1),   false,  type_of),
    e("coalesce",          Arity::AtLeast(1), false,  coalesce),
    e("nullif",            Arity::Exact(2),   false,  nullif),
    e("ifnull",            Arity::Exact(2),   false,  ifnull),
    e("nvl",               Arity::Exact(2),   false,  ifnull),
    e("iif",               Arity::Exact(3),   false,  iif),
    // -- type conversion (constructor casts; strict — a null casts to null) --
    e("int",               Arity::Exact(1),   true,   cast_int),
    e("float",             Arity::Exact(1),   true,   cast_float),
    e("string",            Arity::Exact(1),   true,   cast_string),
    e("bool",              Arity::Exact(1),   true,   cast_bool),
    e("number",            Arity::Exact(1),   true,   cast_number),
    // -- math ----------------------------------------------------------------
    e("abs",               Arity::Exact(1),   true,   abs),
    e("ceil",              Arity::Exact(1),   true,   ceil),
    e("ceiling",           Arity::Exact(1),   true,   ceil),
    e("floor",             Arity::Exact(1),   true,   floor),
    e("trunc",             Arity::Exact(1),   true,   trunc),
    e("round",             Arity::Range(1, 2),true,   round),
    e("sign",              Arity::Exact(1),   true,   sign),
    e("sqrt",              Arity::Exact(1),   true,   sqrt),
    e("pow",               Arity::Exact(2),   true,   pow),
    e("power",             Arity::Exact(2),   true,   pow),
    e("exp",               Arity::Exact(1),   true,   exp),
    e("ln",                Arity::Exact(1),   true,   ln),
    e("log10",             Arity::Exact(1),   true,   log10),
    e("mod",               Arity::Exact(2),   true,   modulo),
    e("greatest",          Arity::AtLeast(1), true,   greatest),
    e("least",             Arity::AtLeast(1), true,   least),
    // -- string  ((*) = dynamic on value) ------------------------------------
    e("length",            Arity::Exact(1),   true,   length),       // (*)
    e("upper",             Arity::Exact(1),   true,   upper),
    e("lower",             Arity::Exact(1),   true,   lower),
    e("trim",              Arity::Exact(1),   true,   trim),
    e("ltrim",             Arity::Exact(1),   true,   ltrim),
    e("rtrim",             Arity::Exact(1),   true,   rtrim),
    e("substr",            Arity::Range(2, 3),true,   substr),
    e("substring",         Arity::Range(2, 3),true,   substr),
    e("replace",           Arity::Exact(3),   true,   replace),
    e("concat",            Arity::AtLeast(1), false,  concat),       // null-skipping
    e("concat_ws",         Arity::AtLeast(1), false,  concat_ws),    // null-skipping
    e("repeat",            Arity::Exact(2),   true,   repeat),
    e("reverse",           Arity::Exact(1),   true,   reverse),      // (*)
    e("lpad",              Arity::Range(2, 3),true,   lpad),
    e("rpad",              Arity::Range(2, 3),true,   rpad),
    e("strpos",            Arity::Exact(2),   true,   strpos),
    e("instr",             Arity::Exact(2),   true,   strpos),
    e("starts_with",       Arity::Exact(2),   true,   starts_with),
    e("ends_with",         Arity::Exact(2),   true,   ends_with),
    e("contains",          Arity::Exact(2),   true,   contains),     // (*)
    // -- array ---------------------------------------------------------------
    e("array_length",      Arity::Exact(1),   true,   array_length),
    e("array_contains",    Arity::Exact(2),   true,   array_contains),
    e("array_position",    Arity::Exact(2),   true,   array_position),
    e("array_append",      Arity::Exact(2),   true,   array_append),
    e("array_prepend",     Arity::Exact(2),   true,   array_prepend),
    e("array_concat",      Arity::Exact(2),   true,   array_concat),
    e("array_reverse",     Arity::Exact(1),   true,   array_reverse),
    e("array_distinct",    Arity::Exact(1),   true,   array_distinct),
    e("array_to_string",   Arity::Exact(2),   true,   array_to_string),
    e("array_slice",       Arity::Exact(3),   true,   array_slice),
    // -- object --------------------------------------------------------------
    e("object_keys",       Arity::Exact(1),   true,   object_keys),
    e("object_values",     Arity::Exact(1),   true,   object_values),
    e("object_has_key",    Arity::Exact(2),   true,   object_has_key),
    // -- file I/O ------------------------------------------------------------
    e("read_csv",          Arity::Range(1, 2),true,   read_csv),
    e("read_jsonl",        Arity::Range(1, 2),true,   read_jsonl),
    e("read_ndjson",       Arity::Range(1, 2),true,   read_jsonl),
    e("read_json",         Arity::Range(1, 2),true,   read_json),
    e("write_csv",         Arity::Exact(3),   true,   write_csv),
    e("write_jsonl",       Arity::Exact(3),   true,   write_jsonl),
    e("write_json",        Arity::Exact(3),   true,   write_json),
];

/// Builds a registry entry (keeps the `BUILTINS` table aligned and terse).
const fn e(name: &'static str, arity: Arity, strict: bool, func: fn(&[Value]) -> Result<Value>) -> Builtin {
    Builtin { name, arity, strict, func }
}

//------------------------------
// Registry interface
//------------------------------

/// Resolves a builtin by name to its registry index, or `None` if undefined.
/// Called at compile time; the index is baked into `Vop::Call`.
pub fn lookup(name: &str) -> Option<usize> {
    BUILTINS.iter().position(|b| b.name == name)
}

/// Whether `argc` satisfies builtin `id`'s arity (a compile-time check).
pub fn arity_ok(id: usize, argc: usize) -> bool {
    BUILTINS[id].arity.ok(argc)
}

/// Invokes builtin `id` with `args`. A strict builtin short-circuits to `null`
/// when any argument is `null`, so its `fn` only ever sees non-null arguments.
pub fn call(id: usize, args: &[Value]) -> Result<Value> {
    let builtin = &BUILTINS[id];
    if builtin.strict && args.iter().any(Value::is_null) {
        return Ok(Value::Null);
    }
    // Builtins operate on owned values (their helpers match `Value::Array`/
    // `Object`/`String` directly), so materialize any flat-backed `Raw` argument
    // first. This is a no-op for the common case — navigated scalars are already
    // owned — and only allocates when a whole stored container is passed in.
    if args.iter().any(|a| matches!(a, Value::Raw(_))) {
        let owned: Vec<Value> = args.iter().cloned().map(Value::materialized).collect();
        return (builtin.func)(&owned);
    }
    (builtin.func)(args)
}

//------------------------------
// Shared helpers
//------------------------------

/// Upper bound on the length of a string a builtin will construct. `repeat` and
/// `lpad`/`rpad` reject larger sizes rather than attempt a pathological
/// allocation from an attacker-sized count — the same "reject rather than
/// produce" stance as the arithmetic overflow / non-finite guards. Mirrors the
/// `SQLITE_LIMIT_LENGTH` cap (≈1 GiB default).
const MAX_STR_LEN: usize = 1 << 30;

/// Wraps a `&str`/`String` as a `Value::String`.
fn text(s: impl AsRef<str>) -> Value {
    Value::String(Rc::from(s.as_ref()))
}

/// A "function `name` got the wrong argument type" runtime error.
fn type_err(name: &str, want: &str, got: &Value) -> Error {
    Error::InternalError(format!("{name}() requires {want}, got {got}"))
}

/// Coerces a numeric argument to `f64`, else a type error.
fn want_num(name: &str, v: &Value) -> Result<f64> {
    v.to_float().map_err(|_| type_err(name, "a number", v))
}

/// Coerces an integral argument to `i64` (floats truncate, strings parse), else
/// a type error.
fn want_int(name: &str, v: &Value) -> Result<i64> {
    v.to_int().map_err(|_| type_err(name, "an integer", v))
}

/// Coerces a string argument to `&str`, else a type error.
fn want_str<'a>(name: &str, v: &'a Value) -> Result<&'a str> {
    v.as_string().map_err(|_| type_err(name, "a string", v))
}

/// Coerces an array argument to `&Vec<Value>`, else a type error.
fn want_arr<'a>(name: &str, v: &'a Value) -> Result<&'a Vec<Value>> {
    match v {
        Value::Array(a) => Ok(a),
        _ => Err(type_err(name, "an array", v)),
    }
}

/// Wraps an `f64` result, rejecting non-finite values (the JSON encoding and the
/// `Float` invariant both forbid NaN/∞), mirroring the arithmetic policy.
fn finite(name: &str, f: f64) -> Result<Value> {
    if f.is_finite() {
        Ok(Value::Float(f))
    } else {
        Err(Error::InternalError(format!(
            "{name}() produced a non-finite result"
        )))
    }
}

/// The runtime type name of a value (matches `Value`'s variants).
fn type_name(v: &Value) -> &'static str {
    v.type_name()
}

/// Renders a scalar for `concat`/`array_to_string`: strings stay raw, everything
/// else uses its JSON form (`42`, `true`, `[1,2]`).
fn stringify(v: &Value) -> String {
    match v.as_string() {
        Ok(s) => s.to_string(),
        Err(_) => v.to_string(),
    }
}

//------------------------------
// Type / conditional (null-aware)
//------------------------------

/// Returns the argument's runtime type name as a string.
fn type_of(args: &[Value]) -> Result<Value> {
    Ok(text(type_name(&args[0])))
}

/// Returns the first non-null argument, or null if all are null.
fn coalesce(args: &[Value]) -> Result<Value> {
    Ok(args
        .iter()
        .find(|v| !v.is_null())
        .cloned()
        .unwrap_or(Value::Null))
}

/// Returns null when the two arguments are equal, else the first argument.
fn nullif(args: &[Value]) -> Result<Value> {
    if args[0] == args[1] {
        Ok(Value::Null)
    } else {
        Ok(args[0].clone())
    }
}

/// Returns the second argument when the first is null, else the first.
fn ifnull(args: &[Value]) -> Result<Value> {
    if args[0].is_null() {
        Ok(args[1].clone())
    } else {
        Ok(args[0].clone())
    }
}

/// Returns the second argument when the condition is truthy, else the third.
fn iif(args: &[Value]) -> Result<Value> {
    if args[0].is_truthy() {
        Ok(args[1].clone())
    } else {
        Ok(args[2].clone())
    }
}

//------------------------------
// Cast (type conversion)
//------------------------------

/// Casts to int: truncates toward zero, parsing strings leniently.
fn cast_int(args: &[Value]) -> Result<Value> {
    Ok(Value::Int(args[0].to_int()?))
}

/// Casts to float: widens numbers, parses strings (rejecting a non-finite result).
fn cast_float(args: &[Value]) -> Result<Value> {
    finite("float", args[0].to_float()?)
}

/// Casts to string: a scalar's text form (strings raw, other scalars their JSON
/// form). Arrays, objects, and other non-scalars are not convertible.
fn cast_string(args: &[Value]) -> Result<Value> {
    Ok(text(args[0].to_text()?))
}

/// Casts to bool: zero is false and nonzero true; the strings `true`/`false`
/// (case-insensitive) map to their values, anything else errors.
fn cast_bool(args: &[Value]) -> Result<Value> {
    Ok(Value::Bool(args[0].to_bool()?))
}

/// Casts to number: numbers and bools map to themselves / 0 / 1, and a string
/// parses exactly as the matching numeric literal would ([`Value::number`]),
/// rejecting a non-numeric (non-finite) result.
fn cast_number(args: &[Value]) -> Result<Value> {
    args[0].to_number()
}

//------------------------------
// Math
//------------------------------

/// Absolute value; ints stay ints (overflow on `i64::MIN` errors).
fn abs(args: &[Value]) -> Result<Value> {
    match &args[0] {
        Value::Int(i) => i
            .checked_abs()
            .map(Value::Int)
            .ok_or_else(|| Error::InternalError("integer overflow in abs()".into())),
        Value::Float(f) => Ok(Value::Float(f.abs())),
        v => Err(type_err("abs", "a number", v)),
    }
}

/// Applies a float rounding function; an int argument passes through unchanged.
fn round_to(name: &str, arg: &Value, f: impl Fn(f64) -> f64) -> Result<Value> {
    match arg {
        Value::Int(i) => Ok(Value::Int(*i)),
        Value::Float(x) => finite(name, f(*x)),
        v => Err(type_err(name, "a number", v)),
    }
}

/// Smallest integer ≥ the argument.
fn ceil(args: &[Value]) -> Result<Value> {
    round_to("ceil", &args[0], f64::ceil)
}

/// Largest integer ≤ the argument.
fn floor(args: &[Value]) -> Result<Value> {
    round_to("floor", &args[0], f64::floor)
}

/// Truncates toward zero.
fn trunc(args: &[Value]) -> Result<Value> {
    round_to("trunc", &args[0], f64::trunc)
}

/// Rounds to the nearest integer, or to `n` decimal places with a second arg.
fn round(args: &[Value]) -> Result<Value> {
    if args.len() == 1 {
        return round_to("round", &args[0], f64::round);
    }
    let places = want_int("round", &args[1])?;
    match &args[0] {
        Value::Int(i) => Ok(Value::Int(*i)),
        Value::Float(x) => {
            let scale = 10f64.powi(places as i32);
            finite("round", (x * scale).round() / scale)
        }
        v => Err(type_err("round", "a number", v)),
    }
}

/// Sign of the argument as -1, 0, or 1 (preserving int/float).
fn sign(args: &[Value]) -> Result<Value> {
    match &args[0] {
        Value::Int(i) => Ok(Value::Int(i.signum())),
        Value::Float(f) => {
            let s = if *f > 0.0 {
                1.0
            } else if *f < 0.0 {
                -1.0
            } else {
                0.0
            };
            Ok(Value::Float(s))
        }
        v => Err(type_err("sign", "a number", v)),
    }
}

/// Square root; a negative argument errors (no NaN).
fn sqrt(args: &[Value]) -> Result<Value> {
    let x = want_num("sqrt", &args[0])?;
    if x < 0.0 {
        return Err(Error::InternalError("sqrt() of a negative number".into()));
    }
    finite("sqrt", x.sqrt())
}

/// Raises the base to the exponent; non-negative integer powers stay ints.
fn pow(args: &[Value]) -> Result<Value> {
    if let (Value::Int(base), Value::Int(exp)) = (&args[0], &args[1])
        && let Ok(exp) = u32::try_from(*exp)
        && let Some(r) = base.checked_pow(exp)
    {
        return Ok(Value::Int(r));
    }
    let base = want_num("pow", &args[0])?;
    let exp = want_num("pow", &args[1])?;
    finite("pow", base.powf(exp))
}

/// e raised to the argument.
fn exp(args: &[Value]) -> Result<Value> {
    finite("exp", want_num("exp", &args[0])?.exp())
}

/// Natural logarithm; a non-positive argument errors.
fn ln(args: &[Value]) -> Result<Value> {
    let x = want_num("ln", &args[0])?;
    if x <= 0.0 {
        return Err(Error::InternalError("ln() requires a positive number".into()));
    }
    finite("ln", x.ln())
}

/// Base-10 logarithm; a non-positive argument errors.
fn log10(args: &[Value]) -> Result<Value> {
    let x = want_num("log10", &args[0])?;
    if x <= 0.0 {
        return Err(Error::InternalError(
            "log10() requires a positive number".into(),
        ));
    }
    finite("log10", x.log10())
}

/// Remainder of the first argument divided by the second (errors on zero).
fn modulo(args: &[Value]) -> Result<Value> {
    args[0].clone().rem(&args[1])
}

/// Largest of the arguments (by value order); incomparable args error.
fn greatest(args: &[Value]) -> Result<Value> {
    extremum("greatest", args, Ordering::Greater)
}

/// Smallest of the arguments (by value order); incomparable args error.
fn least(args: &[Value]) -> Result<Value> {
    extremum("least", args, Ordering::Less)
}

/// Folds the arguments keeping the one that compares `want` against the rest.
fn extremum(name: &str, args: &[Value], want: Ordering) -> Result<Value> {
    let mut best = 0;
    for (i, v) in args.iter().enumerate().skip(1) {
        match v.partial_cmp(&args[best]) {
            Some(ord) if ord == want => best = i,
            Some(_) => {}
            None => {
                return Err(Error::InternalError(format!(
                    "{name}() arguments are not comparable"
                )));
            }
        }
    }
    Ok(args[best].clone())
}

//------------------------------
// String  ((*) = dynamic on value)
//------------------------------

/// Length: characters of a string, elements of an array, or members of an
/// object. (*)
fn length(args: &[Value]) -> Result<Value> {
    let n = match &args[0] {
        Value::String(s) => s.chars().count(),
        Value::Array(a) => a.len(),
        Value::Object(o) => o.len(),
        v => return Err(type_err("length", "a string, array, or object", v)),
    };
    Ok(Value::Int(n as i64))
}

/// Uppercases a string.
fn upper(args: &[Value]) -> Result<Value> {
    Ok(text(want_str("upper", &args[0])?.to_uppercase()))
}

/// Lowercases a string.
fn lower(args: &[Value]) -> Result<Value> {
    Ok(text(want_str("lower", &args[0])?.to_lowercase()))
}

/// Strips leading and trailing whitespace.
fn trim(args: &[Value]) -> Result<Value> {
    Ok(text(want_str("trim", &args[0])?.trim()))
}

/// Strips leading whitespace.
fn ltrim(args: &[Value]) -> Result<Value> {
    Ok(text(want_str("ltrim", &args[0])?.trim_start()))
}

/// Strips trailing whitespace.
fn rtrim(args: &[Value]) -> Result<Value> {
    Ok(text(want_str("rtrim", &args[0])?.trim_end()))
}

/// Substring from a 1-based start, with an optional character length.
fn substr(args: &[Value]) -> Result<Value> {
    let chars: Vec<char> = want_str("substr", &args[0])?.chars().collect();
    let len = chars.len();
    let start = want_int("substr", &args[1])?;
    let begin = if start < 1 { 0 } else { (start - 1) as usize };
    if begin >= len {
        return Ok(text(""));
    }
    let end = if args.len() == 3 {
        let count = want_int("substr", &args[2])?;
        if count <= 0 {
            return Ok(text(""));
        }
        (begin + count as usize).min(len)
    } else {
        len
    };
    Ok(text(chars[begin..end].iter().collect::<String>()))
}

/// Replaces every occurrence of a substring (an empty search is a no-op).
fn replace(args: &[Value]) -> Result<Value> {
    let s = want_str("replace", &args[0])?;
    let from = want_str("replace", &args[1])?;
    let to = want_str("replace", &args[2])?;
    if from.is_empty() {
        return Ok(text(s));
    }
    Ok(text(s.replace(from, to)))
}

/// Concatenates the arguments, skipping nulls and stringifying scalars.
fn concat(args: &[Value]) -> Result<Value> {
    let mut out = String::new();
    for v in args {
        if !v.is_null() {
            out.push_str(&stringify(v));
        }
    }
    Ok(text(out))
}

/// Joins the trailing arguments with a separator, skipping nulls. A null
/// separator yields null.
fn concat_ws(args: &[Value]) -> Result<Value> {
    if args[0].is_null() {
        return Ok(Value::Null);
    }
    let sep = want_str("concat_ws", &args[0])?;
    let parts: Vec<String> = args[1..]
        .iter()
        .filter(|v| !v.is_null())
        .map(stringify)
        .collect();
    Ok(text(parts.join(sep)))
}

/// Repeats a string `n` times (`n ≤ 0` yields the empty string).
fn repeat(args: &[Value]) -> Result<Value> {
    let s = want_str("repeat", &args[0])?;
    let n = want_int("repeat", &args[1])?;
    if n <= 0 {
        return Ok(text(""));
    }
    let n = n as usize;
    match s.len().checked_mul(n) {
        Some(len) if len <= MAX_STR_LEN => Ok(text(s.repeat(n))),
        _ => Err(Error::InternalError("repeat() result is too large".into())),
    }
}

/// Reverses a string's characters or an array's elements. (*)
fn reverse(args: &[Value]) -> Result<Value> {
    match &args[0] {
        Value::String(s) => Ok(text(s.chars().rev().collect::<String>())),
        Value::Array(a) => Ok(Value::Array(Rc::new(a.iter().rev().cloned().collect()))),
        v => Err(type_err("reverse", "a string or array", v)),
    }
}

/// Pads a string to a target character width, on the left (`left`) or right,
/// truncating if it is already longer. The fill defaults to a space.
fn pad(name: &str, args: &[Value], left: bool) -> Result<Value> {
    let s = want_str(name, &args[0])?;
    let width = want_int(name, &args[1])?;
    if width <= 0 {
        return Ok(text(""));
    }
    let width = width as usize;
    if width > MAX_STR_LEN {
        return Err(Error::InternalError(format!("{name}() result is too large")));
    }
    let chars: Vec<char> = s.chars().collect();
    if chars.len() >= width {
        return Ok(text(chars[..width].iter().collect::<String>()));
    }
    let fill: Vec<char> = if args.len() == 3 {
        want_str(name, &args[2])?.chars().collect()
    } else {
        vec![' ']
    };
    if fill.is_empty() {
        return Ok(text(s));
    }
    let padding: String = fill.iter().cycle().take(width - chars.len()).collect();
    Ok(text(if left {
        format!("{padding}{s}")
    } else {
        format!("{s}{padding}")
    }))
}

/// Left-pads a string to a target width.
fn lpad(args: &[Value]) -> Result<Value> {
    pad("lpad", args, true)
}

/// Right-pads a string to a target width.
fn rpad(args: &[Value]) -> Result<Value> {
    pad("rpad", args, false)
}

/// 1-based index of the first occurrence of a substring, or 0 if absent.
fn strpos(args: &[Value]) -> Result<Value> {
    let s = want_str("strpos", &args[0])?;
    let sub = want_str("strpos", &args[1])?;
    if sub.is_empty() {
        return Ok(Value::Int(1));
    }
    match s.find(sub) {
        Some(byte) => Ok(Value::Int((s[..byte].chars().count() + 1) as i64)),
        None => Ok(Value::Int(0)),
    }
}

/// Whether a string begins with a prefix.
fn starts_with(args: &[Value]) -> Result<Value> {
    let s = want_str("starts_with", &args[0])?;
    let prefix = want_str("starts_with", &args[1])?;
    Ok(Value::Bool(s.starts_with(prefix)))
}

/// Whether a string ends with a suffix.
fn ends_with(args: &[Value]) -> Result<Value> {
    let s = want_str("ends_with", &args[0])?;
    let suffix = want_str("ends_with", &args[1])?;
    Ok(Value::Bool(s.ends_with(suffix)))
}

/// Whether a string contains a substring, or an array contains an element. (*)
fn contains(args: &[Value]) -> Result<Value> {
    match &args[0] {
        Value::String(s) => {
            let sub = want_str("contains", &args[1])?;
            Ok(Value::Bool(s.contains(sub)))
        }
        Value::Array(a) => Ok(Value::Bool(a.iter().any(|x| x == &args[1]))),
        v => Err(type_err("contains", "a string or array", v)),
    }
}

//------------------------------
// Array
//------------------------------

/// Number of elements in an array.
fn array_length(args: &[Value]) -> Result<Value> {
    Ok(Value::Int(want_arr("array_length", &args[0])?.len() as i64))
}

/// Whether an array contains an element (by value equality).
fn array_contains(args: &[Value]) -> Result<Value> {
    let a = want_arr("array_contains", &args[0])?;
    Ok(Value::Bool(a.iter().any(|x| x == &args[1])))
}

/// 1-based index of an element in an array, or 0 if absent.
fn array_position(args: &[Value]) -> Result<Value> {
    let a = want_arr("array_position", &args[0])?;
    let pos = a.iter().position(|x| x == &args[1]);
    Ok(Value::Int(pos.map_or(0, |i| (i + 1) as i64)))
}

/// Returns the array with an element appended.
fn array_append(args: &[Value]) -> Result<Value> {
    let mut items = want_arr("array_append", &args[0])?.clone();
    items.push(args[1].clone());
    Ok(Value::Array(Rc::new(items)))
}

/// Returns the array (second arg) with an element prepended.
fn array_prepend(args: &[Value]) -> Result<Value> {
    let a = want_arr("array_prepend", &args[1])?;
    let mut items = Vec::with_capacity(a.len() + 1);
    items.push(args[0].clone());
    items.extend(a.iter().cloned());
    Ok(Value::Array(Rc::new(items)))
}

/// Concatenates two arrays.
fn array_concat(args: &[Value]) -> Result<Value> {
    let mut items = want_arr("array_concat", &args[0])?.clone();
    items.extend(want_arr("array_concat", &args[1])?.iter().cloned());
    Ok(Value::Array(Rc::new(items)))
}

/// Returns the array with its elements reversed.
fn array_reverse(args: &[Value]) -> Result<Value> {
    let a = want_arr("array_reverse", &args[0])?;
    Ok(Value::Array(Rc::new(a.iter().rev().cloned().collect())))
}

/// Returns the array with duplicate elements removed, keeping first occurrence.
fn array_distinct(args: &[Value]) -> Result<Value> {
    let a = want_arr("array_distinct", &args[0])?;
    let mut out = Vec::new();
    for x in a {
        if !out.iter().any(|y| y == x) {
            out.push(x.clone());
        }
    }
    Ok(Value::Array(Rc::new(out)))
}

/// Joins an array's non-null elements into a string with a separator.
fn array_to_string(args: &[Value]) -> Result<Value> {
    let a = want_arr("array_to_string", &args[0])?;
    let sep = want_str("array_to_string", &args[1])?;
    let parts: Vec<String> = a.iter().filter(|v| !v.is_null()).map(stringify).collect();
    Ok(text(parts.join(sep)))
}

/// Sub-array between 1-based inclusive `start` and `end`, clamped to bounds.
fn array_slice(args: &[Value]) -> Result<Value> {
    let a = want_arr("array_slice", &args[0])?;
    let start = want_int("array_slice", &args[1])?;
    let end = want_int("array_slice", &args[2])?;
    let begin = if start < 1 { 0 } else { (start - 1) as usize };
    let finish = if end < 0 { 0 } else { (end as usize).min(a.len()) };
    if begin >= finish {
        return Ok(Value::array());
    }
    Ok(Value::Array(Rc::new(a[begin..finish].to_vec())))
}

//------------------------------
// Object
//------------------------------

/// The object's keys as a string array, in insertion order.
fn object_keys(args: &[Value]) -> Result<Value> {
    match &args[0] {
        Value::Object(o) => Ok(Value::Array(Rc::new(o.iter().map(|(k, _)| text(k)).collect()))),
        v => Err(type_err("object_keys", "an object", v)),
    }
}

/// The object's values as an array, in insertion order.
fn object_values(args: &[Value]) -> Result<Value> {
    match &args[0] {
        Value::Object(o) => Ok(Value::Array(Rc::new(
            o.iter().map(|(_, v)| v.clone()).collect(),
        ))),
        v => Err(type_err("object_values", "an object", v)),
    }
}

/// Whether the object has a member under the given key.
fn object_has_key(args: &[Value]) -> Result<Value> {
    match &args[0] {
        Value::Object(o) => {
            let key = want_str("object_has_key", &args[1])?;
            Ok(Value::Bool(o.get(key).is_some()))
        }
        v => Err(type_err("object_has_key", "an object", v)),
    }
}

//------------------------------
// File I/O
//------------------------------

fn read_opts(args: &[Value]) -> Result<ReadOptions> {
    match args.len() {
        1 => Ok(ReadOptions::default()),
        2 => ReadOptions::from_value(&args[1]),
        _ => unreachable!(),
    }
}

fn write_opts(args: &[Value]) -> Result<WriteOptions> {
    WriteOptions::from_value(&args[2])
}

/// Reads a CSV/TSV file into an array of row objects.
fn read_csv(args: &[Value]) -> Result<Value> {
    let path = want_str("read_csv", &args[0])?;
    let opts = read_opts(args)?;
    let format = read::infer_format(path).unwrap_or(FileFormat::Csv);
    // `open_rows` already applies the TSV delimiter for `FileFormat::Tsv`.
    let rows = read::read_rows(path, format, opts)?;
    Ok(Value::Array(Rc::new(rows)))
}

/// Reads a JSONL file into an array of row objects.
fn read_jsonl(args: &[Value]) -> Result<Value> {
    let path = want_str("read_jsonl", &args[0])?;
    let opts = read_opts(args)?;
    let rows = read::read_rows(path, FileFormat::Jsonl, opts)?;
    Ok(Value::Array(Rc::new(rows)))
}

/// Reads a JSON document into an array of rows.
fn read_json(args: &[Value]) -> Result<Value> {
    let path = want_str("read_json", &args[0])?;
    let opts = read_opts(args)?;
    let rows = read::read_rows(path, FileFormat::Json, opts)?;
    Ok(Value::Array(Rc::new(rows)))
}

/// Writes rows to a JSON file as a single top-level array; returns null.
fn write_json(args: &[Value]) -> Result<Value> {
    let path = want_str("write_json", &args[0])?;
    let rows = want_arr("write_json", &args[1])?;
    let opts = write_opts(args)?;
    read::write_rows(path, FileFormat::Json, opts, rows)?;
    Ok(Value::Null)
}

/// Writes row objects to a CSV/TSV file; returns null.
fn write_csv(args: &[Value]) -> Result<Value> {
    let path = want_str("write_csv", &args[0])?;
    let rows = want_arr("write_csv", &args[1])?;
    let opts = write_opts(args)?;
    let format = read::infer_format(path).unwrap_or(FileFormat::Csv);
    let opts = if format == FileFormat::Tsv {
        opts.for_tsv()
    } else {
        opts
    };
    read::write_rows(path, format, opts, rows)?;
    Ok(Value::Null)
}

/// Writes row objects to a JSONL file; returns null.
fn write_jsonl(args: &[Value]) -> Result<Value> {
    let path = want_str("write_jsonl", &args[0])?;
    let rows = want_arr("write_jsonl", &args[1])?;
    let opts = write_opts(args)?;
    read::write_rows(path, FileFormat::Jsonl, opts, rows)?;
    Ok(Value::Null)
}
