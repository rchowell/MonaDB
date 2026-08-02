//! The AST — the shape the parser produces.
//!
//! Defines the statement, expression, and type nodes the grammar builds, along with
//! the `#[inline]` action functions the LALRPOP rules call to construct them. The
//! binder mutates this tree in place; the compiler consumes it to emit `Vop` bytecode.

use std::vec;

use crate::read::FileSource;
use crate::value::Value;

pub use crate::display::ToSql;

/// A top-level SQL statement that the compiler turns into a `Program`.
#[derive(Debug, Clone)]
pub enum Statement {
    Begin,
    Clear(Clear),
    Commit,
    Copy(Copy),
    Create(Create),
    Delete(Delete),
    Drop(Drop),
    Insert(Insert),
    Rollback,
    Select(Select),
}

/// A CREATE statement.
#[derive(Debug, Clone)]
#[allow(clippy::large_enum_variant)]
pub enum Create {
    /// `CREATE TABLE` with an optional key declaration.
    Table(TableDefinition),
    /// `CREATE TABLE … AS SELECT`.
    TableAs {
        table: TableDefinition,
        select: Select,
    },
}

/// A COPY import/export statement.
#[derive(Debug, Clone)]
#[allow(clippy::large_enum_variant)]
pub enum Copy {
    /// `COPY <table> FROM <file> [<options>]`.
    From {
        target: TableDefinition,
        path: String,
        options: Obj,
    },
    /// `COPY <source> TO <file> [<options>]`.
    To {
        source: CopySource,
        path: String,
        options: Obj,
    },
}

/// The data source for `COPY … TO`.
#[derive(Debug, Clone)]
#[allow(clippy::large_enum_variant)]
pub enum CopySource {
    /// A table name.
    Table {
        name: String,
        oid: Option<u32>,
    },
    /// A parenthesized SELECT.
    Query(Select),
}

/// An INSERT of one or more row expressions into a table.
#[derive(Debug, Clone)]
pub struct Insert {
    pub target: TableDefinition,
    pub source: Vec<Expr>,
}

/// A DELETE of the `from` rows matching the optional `where_`.
#[derive(Debug, Clone)]
pub struct Delete {
    pub from: From,
    pub where_: Option<Where>,
}

/// A DROP TABLE of the named table.
#[derive(Debug, Clone)]
pub struct Drop {
    pub name: String,
    pub oid: Option<u32>, // set by binder
}

/// A CLEAR, emptying the named table's rows but keeping its definition.
#[derive(Debug, Clone)]
pub struct Clear {
    pub name: String,
    pub oid: Option<u32>, // set by binder
}

//------------------------------
// Table Definition
//------------------------------

/// A table's name and its declared key columns (the composite key, in order).
#[derive(Debug, PartialEq, Clone)]
pub struct TableDefinition {
    pub oid: Option<u32>, // set by binder
    pub name: String,
    pub keys: Vec<Key>,
}

/// One key column: a name and its declared type (int or string).
#[derive(Debug, PartialEq, Clone)]
pub struct Key {
    pub name: String,
    pub ty: Type,
}

//------------------------------
// DQL
//------------------------------

/// A SELECT query: its from-sources, residual filter, grouping, group filter,
/// order, limit, and projection. Clauses run in spec order —
/// from → where → group → having → order → limit → select.
#[derive(Debug, Clone, PartialEq)]
pub struct Select {
    pub from: Vec<From>,
    // pub with: Option<Expr>,
    pub where_: Option<Where>,
    pub group: Option<GroupBy>,
    pub having: Option<Where>,
    pub order: Option<OrderBy>,
    pub limit: Option<Limit>,
    pub select: Constructor,
}

/// A GROUP BY clause: its grouping key expressions, most significant first. The
/// post-where stream is sorted by these (then streamed) so each distinct key
/// forms one output row.
#[derive(Debug, Clone, PartialEq)]
pub struct GroupBy {
    pub keys: Vec<Expr>,
}

/// The projection form of a SELECT.
#[derive(Debug, Clone, PartialEq)]
pub enum Constructor {
    /// Identity `.` — project the binding tuple as an object.
    None,
    /// Spread `*` — merge all bindings into one object.
    Star,
    /// A single expression value.
    Expr(Expr),
    /// An explicit `{ k: v, ... }` member list.
    List(Vec<Member>),
    /// `pivot value at name` — fold the whole binding stream into one object,
    /// contributing the member `name: value` for each tuple (the dual of
    /// [`Source::Unpivot`]). The query yields exactly one object.
    Pivot(Pivot),
}

/// The two expressions of a `pivot value at name` projection.
#[derive(Debug, Clone, PartialEq)]
pub struct Pivot {
    /// The attribute value contributed by each binding tuple.
    pub value: ExprRef,
    /// The attribute name contributed by each binding tuple (must be a string).
    pub name: ExprRef,
}

/// A from-clause source: a named table, an evaluated value to iterate, or an
/// `unpivot` over the attribute-value pairs of a tuple.
#[derive(Debug, Clone, PartialEq)]
pub enum Source {
    Table(String),
    Value(Box<Expr>),
    Unpivot(Unpivot),
    /// A keyed-table prefix range, scanned directly off the btree (no array
    /// materialization). The binder lowers a partial-key subscript in FROM
    /// position to this; the compiler encodes its `args`/`keys` into the
    /// leading-key prefix and emits a prefix [`Scan`](crate::vm::Vop::Scan).
    Range(Get),
    /// A file, streamed row by row (no array materialization). The binder
    /// lowers a `read_csv`/`read_jsonl`/… call with a literal path to this, and
    /// the compiler emits a [`ScanFile`](crate::vm::Vop::ScanFile).
    ///
    /// This is a pure optimization of `Value(read_*(…))`: anything that cannot
    /// be resolved at compile time — a runtime path, parameter options, an
    /// unknown extension — stays a [`Source::Value`] and still runs eagerly.
    File(Box<FileSource>),
}

/// An `unpivot expr as value at name` source. It ranges over the attribute-value
/// pairs of the tuple `expr` evaluates to: each pair binds its value under the
/// enclosing [`From::var`] (the `as` alias) and, optionally, its attribute name
/// under [`Unpivot::att`] (the `at` alias). A non-object `expr` yields no rows.
#[derive(Debug, Clone, PartialEq)]
pub struct Unpivot {
    /// The tuple whose attribute-value pairs are iterated.
    pub expr: ExprRef,
    /// The cursor binding the pair's value (the `as` alias), set by the binder.
    pub val_csr: Option<u32>,
    /// The optional `at` alias binding the pair's attribute name.
    pub att: Option<String>,
    /// The cursor binding the attribute name, set by the binder when `att` is set.
    pub att_csr: Option<u32>,
}

/// One from-item: a source bound to an alias, plus binder-assigned slots.
#[derive(Debug, Clone, PartialEq)]
pub struct From {
    pub src: Source,
    pub var: String,      // AS <var>
    pub csr: Option<u32>, // cursor slot, set by binder
    pub oid: Option<u32>, // table oid, set by binder for Table sources
}

/// A WHERE predicate — just an expression evaluated per binding tuple.
pub type Where = Expr;

/// A LIMIT clause as a half-open row range `[skip, skip+take)`.
#[derive(Debug, Clone, PartialEq)]
pub enum Limit {
    /// `limit N..` — skip the first N rows.
    Skip(u64),
    /// `limit N` — take at most N rows.
    Take(u64),
    /// `limit N..M` — skip N, then take up to `M - N`.
    Slice(u64, u64),
}

/// An ORDER BY clause: its sort keys, most significant first.
#[derive(Debug, Clone, PartialEq)]
pub struct OrderBy {
    pub keys: Vec<OrderKey>,
}

/// One ORDER BY key: the sort expression and its direction.
#[derive(Debug, Clone, PartialEq)]
pub struct OrderKey {
    pub expr: Expr,
    /// `true` sorts this key descending; `false` (the default) ascending.
    pub desc: bool,
}

/// Whether a variable reference resolves to a table or a field at a cursor.
#[derive(Debug)]
pub enum Scope {
    Table,
    Field,
}

/// Variable references are either to tables or a field at a cursor.
#[derive(Debug, Clone, PartialEq)]
pub struct Var {
    /// The reference name we get from parsing.
    pub name: String,
    /// The bound cursor slot, or `None` until the binder resolves it.
    pub bind: Option<u32>,
}

impl Var {
    /// Creates an unbound field-first reference.
    pub fn unbound(name: &str) -> Self {
        Self {
            name: name.to_string(),
            bind: None,
        }
    }
}

//------------------------------
// Types
//------------------------------

/// A boxed [`Type`], for the recursive object/array cases.
pub type TypeRef = Box<Type>;

/// A declared column or value type.
#[derive(Debug, PartialEq, Clone)]
pub enum Type {
    Any,
    Bool,
    Int,
    Float,
    Number,
    String,
    Object(TObject),
    Array,
}

/// A structural object type: its named members.
#[derive(Debug, PartialEq, Clone)]
pub struct TObject {
    pub members: Vec<TMember>,
}

/// One member of an object type: a name and its type.
#[derive(Debug, PartialEq, Clone)]
pub struct TMember {
    pub name: String,
    pub ty: TypeRef,
}

//------------------------------
// Expressions
//------------------------------

/// A boxed [`Expr`], for the recursive cases.
pub type ExprRef = Box<Expr>;

/// An expression node — the value-producing core of the AST.
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    /// A builtin operator/function call.
    Call(Call),
    /// Path index `input[i]`.
    Jpi(Jpi),
    /// Path key `input.key`.
    Jpk(Jpk),
    /// Computed path step `input[expr]`.
    Jpe(Jpe),
    /// A literal value.
    Lit(Value),
    /// An object constructor.
    Obj(Obj),
    /// An array constructor.
    Array(Vec<Expr>),
    /// A variable reference.
    Var(Var),
    /// Raw multi-element subscript `base[a, b, ...]` (>= 2 args) straight from
    /// the parser; the binder lowers it (table receiver → `Get`, value → error).
    Subscript(Subscript),
    /// A bound keyed-table point lookup (`table[key, ...]`). The binder builds
    /// this once a subscript's base resolves to a catalog table with a full key.
    Get(Get),
    /// An aggregate term (`count(*)`, `sum(x)`, …). The binder lowers a
    /// recognized aggregate `Call` into this; the compiler assigns its slot.
    Agg(Agg),
    /// A query-parameter placeholder (`?`, `$N`, `$name`). The binder
    /// substitutes it with the bound literal, so it never reaches the compiler.
    Param(Param),
    /// A subquery `(select ...)` evaluated as a bag of rows. The compiler
    /// materializes it to a `Value::Array`; in scalar position it then coerces
    /// the array to a single value. A FROM derived table holds one of these as a
    /// [`Source::Value`] and iterates the array directly.
    Subquery(Box<Select>),
    /// `exists (select ...)` — true iff the subquery yields at least one row.
    Exists(Box<Select>),
    /// A quantified comparison `lhs <op> any/all (select ...)`. `in`/`not in`
    /// over a subquery lower to this (`= any` / `<> all`).
    Quantify(Quantify),
}

/// A quantified comparison against a subquery bag: `lhs <op> any/all (sub)`.
#[derive(Debug, Clone, PartialEq)]
pub struct Quantify {
    pub op: CmpOp,
    /// `true` for `all`, `false` for `any`.
    pub all: bool,
    pub lhs: ExprRef,
    pub sub: Box<Select>,
}

/// A comparison operator carried by a [`Quantify`] (the VM applies it per
/// element under three-valued logic).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CmpOp {
    Lt,
    Le,
    Eq,
    Ne,
    Gt,
    Ge,
}

/// A query-parameter placeholder, resolved to a bound value at run time
/// (`Vop::LoadParam`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Param {
    /// `?` (a parse-assigned 1-based index, in source order) or an explicit
    /// `$N` — resolved from the supplied positional list.
    Numbered(u32),
    /// `$name` — resolved from the supplied named map.
    Named(String),
}

impl std::fmt::Display for Param {
    /// Renders the placeholder in `$`-form, for error messages.
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Param::Numbered(n) => write!(f, "${n}"),
            Param::Named(name) => write!(f, "${name}"),
        }
    }
}

/// The supported aggregate functions. The first five mirror `SQLite`'s aggregate
/// set (`count`/`sum`/`min`/`max`/`avg`); the VM's `Agg*` opcodes branch on it.
/// `First` is compiler-internal (not user-callable): it keeps the first value
/// folded into the accumulator, used by GROUP BY to carry each group's
/// representative row across the group boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AggKind {
    Count,
    Sum,
    Min,
    Max,
    Avg,
    First,
}

/// A bound aggregate term: its kind, its argument (`None` is `count(*)`), and the
/// accumulator slot. The binder lowers an aggregate `Call` to this and the
/// compiler fills `slot` from its `alloc_agg` allocator (like cursor/counter
/// slots), so binding stays free of VM-layout concerns.
#[derive(Debug, Clone, PartialEq)]
pub struct Agg {
    pub kind: AggKind,
    pub arg: Option<ExprRef>,
    pub slot: Option<usize>,
}

/// An object constructor's members.
pub type Obj = Vec<Member>;

/// A builtin call: an operator/function name and its argument expressions.
#[derive(Debug, Clone, PartialEq)]
pub struct Call {
    pub name: String,
    pub args: Vec<Expr>,
}

/// One object-constructor member: a `k: v` assignment or a `...spread`.
#[derive(Debug, Clone, PartialEq)]
pub enum Member {
    Assign(String, Expr),
    Spread(Expr),
}

/// A path index `input[idx]`.
#[derive(Debug, Clone, PartialEq)]
pub struct Jpi {
    pub inp: ExprRef,
    pub idx: usize,
}

/// A path key `input.key`.
#[derive(Debug, Clone, PartialEq)]
pub struct Jpk {
    pub inp: ExprRef,
    pub key: String,
}

/// A computed path step `input[exp]`.
#[derive(Debug, Clone, PartialEq)]
pub struct Jpe {
    pub inp: ExprRef,
    pub exp: ExprRef,
}

/// The raw parser node for a multi-element subscript `base[args...]`. The base
/// kind is unknown at parse time; the binder decides table-get vs value access.
#[derive(Debug, Clone, PartialEq)]
pub struct Subscript {
    pub base: ExprRef,
    pub args: Vec<Expr>,
}

/// A bound keyed-table point lookup. `args` are the key argument expressions in
/// key column order — each a literal (`Expr::Lit`) or a parameter (`Expr::Param`).
/// All-literal keys are encoded into the composite key at compile time; a key
/// with any parameter is encoded at runtime (`Vop::EncodeKeyTuple`).
#[derive(Debug, Clone)]
pub struct Get {
    pub csr: u32,
    pub oid: u32,
    pub keys: Vec<Key>,
    pub args: Vec<Expr>,
}

/// Equality ignores the binder-assigned cursor slot (`csr`), which is allocated
/// fresh per occurrence: two subscripts of the same table and key are equal
/// regardless of where each was lowered. This keeps GROUP BY key matching (which
/// compares `Expr`s structurally) working for keyed-subscript group keys.
impl PartialEq for Get {
    fn eq(&self, other: &Self) -> bool {
        self.oid == other.oid && self.keys == other.keys && self.args == other.args
    }
}

//------------------------------
// Parser Actions
//------------------------------

/// Builds a CREATE TABLE from a table definition.
#[inline]
pub fn create_table(table: TableDefinition) -> Create {
    Create::Table(table)
}

/// Builds a CREATE TABLE AS SELECT.
#[inline]
pub fn create_table_as(def: TableDefinition, query: Select) -> Create {
    Create::TableAs { table: def, select: query }
}

/// Builds a table definition from a name and its key columns.
#[inline]
pub fn table_definition(name: String, members: Vec<Key>) -> TableDefinition {
    TableDefinition {
        oid: None,
        name,
        keys: members,
    }
}

/// Builds one key column from a name and type.
#[inline]
pub fn table_key(name: String, ty: Type) -> Key {
    Key { name, ty }
}

/// Builds an INSERT into the named table from row expressions.
#[inline]
pub fn insert(target: String, source: Vec<Expr>) -> Insert {
    Insert {
        target: TableDefinition {
            oid: None,
            name: target,
            keys: vec![],
        },
        source,
    }
}

/// Builds a DELETE from a table, optional alias, and optional WHERE.
#[inline]
pub fn delete(table: String, alias: Option<String>, where_: Option<Where>) -> Delete {
    let from = From {
        var: alias.unwrap_or_else(|| table.clone()),
        src: Source::Table(table),
        csr: None,
        oid: None,
    };
    Delete { from, where_ }
}

/// Builds a DROP TABLE for the named table.
#[inline]
pub fn drop_table(name: String) -> Drop {
    Drop { name, oid: None }
}

/// Builds a CLEAR for the named table.
#[inline]
pub fn clear_table(name: String) -> Clear {
    Clear { name, oid: None }
}

/// Builds `COPY <table> FROM <file> [<options>]`. The lexer has already decoded
/// the `path` string literal (delimiters stripped, escapes resolved).
#[inline]
pub fn copy_from(target: TableDefinition, path: String, options: Obj) -> Copy {
    Copy::From {
        target,
        path,
        options,
    }
}

/// Builds `COPY <source> TO <file> [<options>]`. The lexer has already decoded
/// the `path` string literal (delimiters stripped, escapes resolved).
#[inline]
pub fn copy_to(source: CopySource, path: String, options: Obj) -> Copy {
    Copy::To {
        source,
        path,
        options,
    }
}

/// Builds a table copy source.
#[inline]
pub fn copy_source_table(name: String) -> CopySource {
    CopySource::Table { name, oid: None }
}

/// Builds a query copy source.
#[inline]
pub fn copy_source_query(select: Select) -> CopySource {
    CopySource::Query(select)
}

/// Builds a from-less `select <value>`.
#[inline]
pub fn select_value(select: Constructor) -> Select {
    Select {
        from: vec![],
        where_: None,
        group: None,
        having: None,
        order: None,
        limit: None,
        select,
    }
}

/// Builds a SELECT, attaching a projection to a parsed from/where/group/having/
/// order/limit block.
#[inline]
pub fn select(select: Constructor, block: Select) -> Select {
    Select {
        from: block.from,
        where_: block.where_,
        group: block.group,
        having: block.having,
        order: block.order,
        limit: block.limit,
        select,
    }
}

/// Builds a `pivot value at name <block>` query: the from/where/group/having/
/// order/limit block keeps its clauses, the projection becomes a
/// [`Constructor::Pivot`].
#[inline]
pub fn pivot(value: Expr, name: Expr, block: Select) -> Select {
    Select {
        from: block.from,
        where_: block.where_,
        group: block.group,
        having: block.having,
        order: block.order,
        limit: block.limit,
        select: Constructor::Pivot(Pivot {
            value: Box::new(value),
            name: Box::new(name),
        }),
    }
}

/// Builds the from/where/group/having/order/limit block; the projection is
/// filled in later.
#[inline]
pub fn select_block(
    from: Vec<From>,
    where_: Option<Where>,
    group: Option<GroupBy>,
    having: Option<Where>,
    order: Option<OrderBy>,
    limit: Option<Limit>,
) -> Select {
    Select {
        from,
        where_,
        group,
        having,
        order,
        limit,
        select: Constructor::None,
    }
}

/// Builds a GROUP BY from its key expressions.
#[inline]
pub fn group_by(keys: Vec<Expr>) -> GroupBy {
    GroupBy { keys }
}

/// Builds an ORDER BY from its keys.
#[inline]
pub fn order_by(keys: Vec<OrderKey>) -> OrderBy {
    OrderBy { keys }
}

/// Builds one ORDER BY key, defaulting to ascending when no direction is given.
#[inline]
pub fn order_key(expr: Expr, desc: Option<bool>) -> OrderKey {
    OrderKey {
        expr,
        desc: desc.unwrap_or(false),
    }
}

/// Builds a projected `expr as name` member.
#[inline]
pub fn select_item(expr: Expr, name: String) -> Member {
    Member::Assign(name, expr)
}

/// Builds a from-item: a bare variable becomes a table reference, a file-path
/// string literal desugars to `read_csv`/`read_jsonl`, any other expression a
/// value source.
///
/// The file case deliberately round-trips through a `read_*` call rather than
/// building a [`Source::File`] here: the binder owns the lowering, so the bare
/// literal and the explicit `read_csv('f.csv', {…})` spelling share one rule
/// and one alias check. Don't "simplify" this into emitting `Source::File`
/// directly — it would duplicate both.
#[inline]
pub fn from_item(src: Expr, alias: Option<String>) -> From {
    match src {
        Expr::Var(var) => From {
            var: alias.unwrap_or_else(|| var.name.clone()),
            src: Source::Table(var.name),
            csr: None,
            oid: None,
        },
        Expr::Lit(Value::String(ref path)) if crate::read::looks_like_file(path) => {
            let var = alias.unwrap_or_else(|| crate::read::default_alias(path));
            From {
                var,
                src: Source::Value(Box::new(desugar_file_from(path))),
                csr: None,
                oid: None,
            }
        }
        expr => From {
            var: alias.unwrap_or_default(),
            src: Source::Value(Box::new(expr)),
            csr: None,
            oid: None,
        },
    }
}

/// Desugars a file-path string literal into a `read_csv` / `read_jsonl` call.
#[inline]
#[allow(clippy::missing_panics_doc)]
pub fn desugar_file_from(path: &str) -> Expr {
    let format = crate::read::infer_format(path).expect("looks_like_file");
    let name = crate::read::read_builtin(format).to_string();
    expr_call(
        name,
        vec![Expr::Lit(Value::String(std::rc::Rc::from(path)))],
    )
}

/// The builtins that name a readable file source.
const READ_BUILTINS: [&str; 5] = [
    "read_csv",
    "read_tsv",
    "read_jsonl",
    "read_ndjson",
    "read_json",
];

/// Folds a literal expression to a runtime [`Value`], or `None` when any part
/// of it is non-literal (a parameter, a binding reference, a call).
///
/// This is the compile-time counterpart of the runtime `Value` that
/// [`ReadOptions::from_value`](crate::read::ReadOptions::from_value) expects,
/// and it is intentionally conservative — anything it declines stays on the
/// eager path.
pub fn const_expr(expr: &Expr) -> Option<Value> {
    match expr {
        Expr::Lit(value) => Some(value.clone()),
        Expr::Array(items) => {
            let mut out = Value::array();
            for item in items {
                out.push(const_expr(item)?);
            }
            Some(out)
        }
        Expr::Obj(members) => {
            let mut out = Value::object();
            for member in members {
                let Member::Assign(name, value) = member else {
                    return None;
                };
                out.set(name.as_str(), const_expr(value)?);
            }
            Some(out)
        }
        _ => None,
    }
}

/// Extracts a compile-time-resolvable [`FileSource`] from a `read_*` call.
///
/// Returns `None` — meaning "leave it eager" — unless *all* of these hold: the
/// callee is a read builtin, its arity is 1 or 2, its path argument is a string
/// literal with a recognized extension, and its options argument (when present)
/// folds to a value that [`ReadOptions`](crate::read::ReadOptions) accepts.
///
/// Declining is never an error: the caller keeps the original
/// [`Source::Value`], which still evaluates correctly through the builtin.
pub fn as_file_source(expr: &Expr) -> Option<FileSource> {
    let Expr::Call(Call { name, args }) = expr else {
        return None;
    };
    if !READ_BUILTINS.contains(&name.as_str()) || args.is_empty() || args.len() > 2 {
        return None;
    }
    let Expr::Lit(Value::String(path)) = &args[0] else {
        return None;
    };
    // The extension is the source of truth for the format, so `read_csv('f.tsv')`
    // still reads tab-separated — matching the eager builtins, which re-infer.
    let format = crate::read::infer_format(path)?;
    let options = match args.get(1) {
        None => crate::read::ReadOptions::default(),
        Some(expr) => crate::read::ReadOptions::from_value(&const_expr(expr)?).ok()?,
    };
    Some(FileSource {
        path: path.to_string(),
        format,
        options,
    })
}

/// Builds an `unpivot expr [as value] [at name]` from-item. The `as` alias
/// becomes the value binding ([`From::var`]); the `at` alias, if present, binds
/// the attribute name.
#[inline]
pub fn unpivot_item(expr: Expr, alias: Option<String>, att: Option<String>) -> From {
    From {
        var: alias.unwrap_or_default(),
        src: Source::Unpivot(Unpivot {
            expr: Box::new(expr),
            val_csr: None,
            att,
            att_csr: None,
        }),
        csr: None,
        oid: None,
    }
}

/// Builds `limit N..` (skip N rows).
#[inline]
pub fn limit_skip(offset: u64) -> Limit {
    Limit::Skip(offset)
}

/// Builds `limit N` (take at most N rows).
#[inline]
pub fn limit_take(limit: u64) -> Limit {
    Limit::Take(limit)
}

/// Builds `limit N..M` (skip N, then take up to `M - N`).
#[inline]
pub fn limit_slice(offset: u64, limit: u64) -> Limit {
    Limit::Slice(offset, limit)
}

// TODO { x, y } => { x: x, y: y } shorthand
// #[inline]
// pub fn member_var(expr: Expr) -> Member {
//     let name = match &expr {
//         Expr::Var(var) => var.clone(),
//         Expr::Jpk(jpk) => jpk.key.clone(),
//         _ => panic!("member_var: {:?}", expr),
//     };
//     Member::Assign(name, expr)
// }

/// Builds a `name: expr` object member.
#[inline]
pub fn member_assign(name: String, expr: Expr) -> Member {
    Member::Assign(name, expr)
}

/// Builds a `...expr` spread member.
#[inline]
pub fn member_spread(expr: Expr) -> Member {
    Member::Spread(expr)
}

//------------------------------
// Parser Actions: Types
//------------------------------

/// Builds the `any` type.
pub fn t_any() -> Type {
    Type::Any
}

/// Builds the `bool` type.
pub fn t_bool() -> Type {
    Type::Bool
}

/// Builds the `int` type.
pub fn t_int() -> Type {
    Type::Int
}

/// Builds the `float` type.
pub fn t_float() -> Type {
    Type::Float
}

/// Builds the `number` type.
pub fn t_number() -> Type {
    Type::Number
}

/// Builds the `string` type.
pub fn t_string() -> Type {
    Type::String
}

/// Builds an object type from its members.
pub fn t_object(members: Vec<TMember>) -> Type {
    Type::Object(TObject { members })
}

/// Builds one object-type member from a name and type.
pub fn t_member(name: String, ty: Type) -> TMember {
    TMember {
        name,
        ty: Box::new(ty),
    }
}

/// Builds the `array` type.
pub fn t_array() -> Type {
    Type::Array
}

//------------------------------
// Parser Actions: Expressions
//------------------------------

/// Builds an unbound variable reference.
#[inline]
#[allow(clippy::needless_pass_by_value)]
pub fn expr_var(name: String) -> Expr {
    Expr::Var(Var::unbound(&name))
}

/// Builds a literal expression.
#[inline]
pub fn expr_lit(val: Value) -> Expr {
    Expr::Lit(val)
}

/// Builds a positional parameter `?`, assigning it the next 1-based index in
/// source order. The parser threads `counter` so indices follow the SQL text,
/// not the binder's traversal order.
#[inline]
pub fn expr_param_positional(counter: &std::cell::Cell<u32>) -> Expr {
    counter.set(counter.get() + 1);
    Expr::Param(Param::Numbered(counter.get()))
}

/// Builds an explicitly numbered parameter `$N` (1-based). An index that
/// overflows `u32` collapses to 0 — the canonical invalid index — which the
/// binder rejects as a missing parameter, a clean bind error rather than an
/// opaque lexer failure.
#[inline]
#[allow(clippy::needless_pass_by_value)] // for .lalrpop
pub fn expr_param_numbered(raw: String) -> Expr {
    Expr::Param(Param::Numbered(raw.parse::<u32>().unwrap_or(0)))
}

/// Builds a named parameter `$name`.
#[inline]
#[allow(clippy::needless_pass_by_value)] // for .lalrpop
pub fn expr_param_named(name: String) -> Expr {
    Expr::Param(Param::Named(name))
}

/// Builds a path-key access `inp.key`.
#[inline]
pub fn expr_jpk(inp: Expr, key: String) -> Expr {
    Expr::Jpk(Jpk {
        inp: Box::new(inp),
        key,
    })
}

/// Builds a computed path access `inp[exp]`.
#[inline]
pub fn expr_jpe(inp: Expr, exp: Expr) -> Expr {
    Expr::Jpe(Jpe {
        inp: Box::new(inp),
        exp: Box::new(exp),
    })
}

/// Builds a multi-element subscript `base[first, rest...]`. The grammar guards
/// arity >= 2 (a single index stays the `Jpe` path-navigation production).
#[inline]
pub fn expr_subscript(base: Expr, first: Expr, rest: Vec<Expr>) -> Expr {
    let mut args = Vec::with_capacity(rest.len() + 1);
    args.push(first);
    args.extend(rest);
    Expr::Subscript(Subscript {
        base: Box::new(base),
        args,
    })
}

/// Builds an object constructor.
#[inline]
pub fn expr_obj(obj: Obj) -> Expr {
    Expr::Obj(obj)
}

/// Builds an array constructor.
#[inline]
pub fn expr_array(items: Vec<Expr>) -> Expr {
    Expr::Array(items)
}

/// Builds a binary operator call `lhs <sym> rhs`.
#[inline]
pub fn expr_binary(sym: &str, lhs: Expr, rhs: Expr) -> Expr {
    Expr::Call(Call {
        name: sym.to_string(),
        args: vec![lhs, rhs],
    })
}

/// Builds a named function call.
#[inline]
pub fn expr_call(name: String, args: Vec<Expr>) -> Expr {
    Expr::Call(Call { name, args })
}

/// Builds a star-call `name(*)`. Only `count(*)` is meaningful, so that lowers
/// straight to an arg-less `Agg`; any other name becomes an arg-less `Call` that
/// the binder rejects (a non-count aggregate, or an unknown function).
#[inline]
pub fn expr_call_star(name: String) -> Expr {
    if name.eq_ignore_ascii_case("count") {
        Expr::Agg(Agg {
            kind: AggKind::Count,
            arg: None,
            slot: None,
        })
    } else {
        Expr::Call(Call { name, args: vec![] })
    }
}

/// Builds a constructor cast `int(expr)` as a call to the per-type conversion
/// builtin (`int`, `float`, …) — like `is not null` desugaring to
/// `not(is_null(...))`, no new AST node or opcode is needed.
#[inline]
pub fn expr_cast(expr: Expr, ty: &Type) -> Expr {
    expr_call(cast_target(ty).to_string(), vec![expr])
}

/// Maps a scalar cast target to its conversion-builtin name.
fn cast_target(ty: &Type) -> &'static str {
    match ty {
        Type::Int => "int",
        Type::Float => "float",
        Type::String => "string",
        Type::Bool => "bool",
        Type::Number => "number",
        // The grammar restricts cast targets to `TScalar`; nothing else reaches here.
        _ => unreachable!("cast target is not a scalar type: {ty:?}"),
    }
}

/// Builds a `not arg` call.
#[inline]
pub fn expr_not(arg: Expr) -> Expr {
    Expr::Call(Call {
        name: "not".to_string(),
        args: vec![arg],
    })
}

/// Builds an `arg is null` test.
#[inline]
pub fn expr_is_null(arg: Expr) -> Expr {
    Expr::Call(Call {
        name: "is_null".to_string(),
        args: vec![arg],
    })
}

/// Builds an `arg is not null` test.
#[inline]
pub fn expr_is_not_null(arg: Expr) -> Expr {
    expr_not(expr_is_null(arg))
}

/// Builds an `arg is true` test.
#[inline]
pub fn expr_is_true(arg: Expr) -> Expr {
    Expr::Call(Call {
        name: "is_true".to_string(),
        args: vec![arg],
    })
}

/// Builds an `arg is false` test.
#[inline]
pub fn expr_is_false(arg: Expr) -> Expr {
    Expr::Call(Call {
        name: "is_false".to_string(),
        args: vec![arg],
    })
}

/// Builds an `arg is unknown` test.
#[inline]
pub fn expr_is_unknown(arg: Expr) -> Expr {
    Expr::Call(Call {
        name: "is_unknown".to_string(),
        args: vec![arg],
    })
}

/// Builds an `arg is not true` test.
#[inline]
pub fn expr_is_not_true(arg: Expr) -> Expr {
    expr_not(expr_is_true(arg))
}

/// Builds an `arg is not false` test.
#[inline]
pub fn expr_is_not_false(arg: Expr) -> Expr {
    expr_not(expr_is_false(arg))
}

/// Builds an `arg is not unknown` test.
#[inline]
pub fn expr_is_not_unknown(arg: Expr) -> Expr {
    expr_not(expr_is_unknown(arg))
}

/// Builds an `x between a and b` test.
#[inline]
pub fn expr_between(x: Expr, a: Expr, b: Expr) -> Expr {
    Expr::Call(Call {
        name: "between".to_string(),
        args: vec![x, a, b],
    })
}

/// Builds an `x not between a and b` test.
#[inline]
pub fn expr_not_between(x: Expr, a: Expr, b: Expr) -> Expr {
    expr_not(expr_between(x, a, b))
}

/// Builds an `x in (list...)` test, with the target as the first argument.
#[inline]
pub fn expr_in_list(x: Expr, list: Vec<Expr>) -> Expr {
    let mut args = Vec::with_capacity(list.len() + 1);
    args.push(x);
    args.extend(list);
    Expr::Call(Call {
        name: "in_list".to_string(),
        args,
    })
}

/// Builds an `x not in (list...)` test.
#[inline]
pub fn expr_not_in_list(x: Expr, list: Vec<Expr>) -> Expr {
    expr_not(expr_in_list(x, list))
}

/// Builds a subquery expression `(select ...)`.
#[inline]
pub fn expr_subquery(select: Select) -> Expr {
    Expr::Subquery(Box::new(select))
}

/// Builds an `exists (select ...)` test.
#[inline]
pub fn expr_exists(select: Select) -> Expr {
    Expr::Exists(Box::new(select))
}

/// Builds a quantified comparison `lhs <op> any/all (select ...)`.
#[inline]
pub fn expr_quantify(lhs: Expr, op: CmpOp, all: bool, select: Select) -> Expr {
    Expr::Quantify(Quantify {
        op,
        all,
        lhs: Box::new(lhs),
        sub: Box::new(select),
    })
}

/// Builds an `x in (select ...)` test as `x = any (select ...)`.
#[inline]
pub fn expr_in_subquery(x: Expr, select: Select) -> Expr {
    expr_quantify(x, CmpOp::Eq, false, select)
}

/// Builds an `x not in (select ...)` test as `x <> all (select ...)`.
#[inline]
pub fn expr_not_in_subquery(x: Expr, select: Select) -> Expr {
    expr_quantify(x, CmpOp::Ne, true, select)
}

#[cfg(test)]
mod test {
    use crate::{lexer::SqlLexer, parser::SqlParser};

    use super::*;

    fn parse(input: &str) -> Statement {
        let l = SqlLexer::new(input);
        let p = SqlParser::new();
        p.parse(&std::cell::Cell::new(0), l).unwrap()
    }

    #[test]
    fn test_acceptance_from() {
        let paths = vec![
            // Table
            "T",
            "Table",
            // // Basic paths
            // "T$.store.book.title",
            // "T$.store['book'].title",
            // "T$.store['book']['title']",
            // "T$.store.book.*",
            // "T$.store.book[0]",
            // "T$.store.book[0].title",
            // "T$.store.book[0]..title",
            // "T$.store.book[0]..*",
            // "T$.store.book[0]..*.*",
            // // Wildcard paths
            // "T$.store.*.title",
            // "T$.store.*[0]",
            // "T$.store.*[0].title",
            // "T$.store.*[0]..title",
            // "T$.store.*[0]..*",
            // "T$.store.*[0]..*.*",
            // // Array indices
            // "T$.store.book[0]",
            // "T$.store.book[1]",
            // "T$.store.book[-1]",
            // "T$.store.book[0,1]",
            // // Array slices
            // // "T$.store.book[0:2]",
            // // "T$.store.book[:2]",
            // // "T$.store.book[1:]",
            // // "T$.store.book[::2]",
            // // Recursive descent
            // "T$..book",
            // "T$..book.title",
            // "T$..book[0]",
            // "T$..book[0].title",
            // "T$..book[0]..title",
            // "T$..book[0]..*",
            // "T$..book[0]..*.*",
            // // Filters
            // "T$.store.book[?(@.price < 10)]",
            // "T$.store.book[?(@.price <= 10)]",
            // "T$.store.book[?(@.price > 10)]",
            // "T$.store.book[?(@.price >= 10)]",
            // "T$.store.book[?(@.price == 10)]",
            // "T$.store.book[?(@.price != 10)]",
            // "T$.store.book[?(@.author == 'John')]",
            // "T$.store.book[?(@.author != 'John')]",
        ];
        // Test each path with an alias
        for path in paths {
            let input = format!("select * from {path} as a;");
            let _ = parse(&input);
        }
        // ok, no panics
    }

    #[test]
    pub fn parse_acceptance_where() {
        let inputs = vec![
            "select * from T where 10;",
            "select * from T where a > 0;",
            // "select * from T where a > 0 and b = 10;",
        ];
        for input in inputs {
            let _ = parse(input);
        }
    }

    #[test]
    fn parse_acceptance_create_table() {
        let cases: &[(&str, &[(&str, Type)])] = &[
            ("create table points;", &[]),
            ("create table points (x int);", &[("x", Type::Int)]),
            (
                "create table points (x int, y int);",
                &[("x", Type::Int), ("y", Type::Int)],
            ),
            ("create table users (id string);", &[("id", Type::String)]),
        ];
        for (input, expected_cols) in cases {
            let stmt = parse(input);
            let Statement::Create(Create::Table(table)) = stmt else {
                panic!("expected create table for {input:?}");
            };
            assert_eq!(table.keys.len(), expected_cols.len(), "input: {input:?}");
            for (actual, (name, ty)) in table.keys.iter().zip(expected_cols.iter()) {
                assert_eq!(actual.name, *name, "input: {input:?}");
                assert_eq!(actual.ty, *ty, "input: {input:?}");
            }
        }
    }

    #[test]
    fn parse_file_io() {
        let cases = [
            "select * from 'tests/fixtures/people.csv';",
            "create table loaded as select * from 'tests/fixtures/people.csv';",
            "copy dst from 'tests/fixtures/people.csv';",
            "copy src to 'tests/fixtures/out.csv';",
            "select * from loaded as r order by r.name;",
        ];
        for input in cases {
            parse(input);
        }
    }

    #[test]
    pub fn parse_acceptance_limit() {
        let inputs = vec![
            "select * from T limit 20;",
            "select * from T limit 10..;",
            "select * from T limit 10..20;",
        ];
        for input in inputs {
            let _ = parse(input);
        }
    }
}
