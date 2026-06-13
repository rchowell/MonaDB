//! AST / IR types and the parser action functions that build them.
//!
//! The grammar in `parser.lalrpop` calls the `#[inline]` functions here to
//! construct nodes; the binder then annotates them (cursor slots, table oids)
//! and the compiler lowers them to bytecode.

use std::vec;

use crate::value::Value;

pub use crate::display::ToSql;

/// A top-level SQL statement — the unit the compiler turns into a `Program`.
#[derive(Debug)]
pub enum Statement {
    Clear(Clear),
    Create(Create),
    Delete(Delete),
    Drop(Drop),
    Insert(Insert),
    Select(Select),
}

/// A CREATE statement. (`CREATE TABLE` is the only form today.)
#[derive(Debug)]
pub enum Create {
    Table(TableDefinition),
}

/// An INSERT of one or more row expressions into a table.
#[derive(Debug)]
pub struct Insert {
    pub target: TableDefinition,
    pub source: Vec<Expr>,
}

/// A DELETE of the `from` rows matching the optional `where_`.
#[derive(Debug)]
pub struct Delete {
    pub from: From,
    pub where_: Option<Where>,
}

/// A DROP TABLE of the named table.
#[derive(Debug)]
pub struct Drop {
    pub name: String,
    pub oid: Option<u32>, // set by binder
}

/// A CLEAR, emptying the named table's rows but keeping its definition.
#[derive(Debug)]
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
#[derive(Debug)]
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
#[derive(Debug)]
pub struct GroupBy {
    pub keys: Vec<Expr>,
}

/// The projection form of a SELECT.
#[derive(Debug, Clone)]
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
#[derive(Debug, Clone)]
pub struct Pivot {
    /// The attribute value contributed by each binding tuple.
    pub value: ExprRef,
    /// The attribute name contributed by each binding tuple (must be a string).
    pub name: ExprRef,
}

/// A from-clause source: a named table, an evaluated value to iterate, or an
/// `unpivot` over the attribute-value pairs of a tuple.
#[derive(Debug)]
pub enum Source {
    Table(String),
    Value(Box<Expr>),
    Unpivot(Unpivot),
}

/// An `unpivot expr as value at name` source. It ranges over the attribute-value
/// pairs of the tuple `expr` evaluates to: each pair binds its value under the
/// enclosing [`From::var`] (the `as` alias) and, optionally, its attribute name
/// under [`Unpivot::att`] (the `at` alias). A non-object `expr` yields no rows.
#[derive(Debug)]
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
#[derive(Debug)]
pub struct From {
    pub src: Source,
    pub var: String,      // AS <var>
    pub csr: Option<u32>, // cursor slot, set by binder
    pub oid: Option<u32>, // table oid, set by binder for Table sources
}

/// A WHERE predicate — just an expression evaluated per binding tuple.
pub type Where = Expr;

/// A LIMIT clause as a half-open row range `[skip, skip+take)`.
#[derive(Debug)]
pub enum Limit {
    /// `limit N..` — skip the first N rows.
    Skip(u64),
    /// `limit N` — take at most N rows.
    Take(u64),
    /// `limit N..M` — skip N, then take up to `M - N`.
    Slice(u64, u64),
}

/// An ORDER BY clause: its sort keys, most significant first.
#[derive(Debug)]
pub struct OrderBy {
    pub keys: Vec<OrderKey>,
}

/// One ORDER BY key: the sort expression and its direction.
#[derive(Debug)]
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
// JSONPath
//------------------------------

/// A JSONPath: a root identifier followed by navigation segments.
#[derive(Debug)]
pub struct Path {
    pub identifier: String,
    pub segments: Vec<Segment>,
}

/// One path segment: a child step (`.x`) or a recursive descent (`..x`).
#[derive(Debug)]
pub enum Segment {
    Child(Vec<Selector>),
    Descd(Vec<Selector>),
}

/// A selector within a segment: a name, a `*` wildcard, or an array index.
#[derive(Debug)]
pub enum Selector {
    Name(String),
    Wildcard,
    Index(usize),
}

//------------------------------
// Expressions
//------------------------------

/// A boxed [`Expr`], for the recursive cases.
pub type ExprRef = Box<Expr>;

/// An expression node — the value-producing core of the IR.
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
}

/// The supported aggregate functions. The first five mirror SQLite's aggregate
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

/// A bound keyed-table point lookup. `args` are the literal key values in key
/// column order; the compiler encodes them into the composite key (Task 3).
#[derive(Debug, Clone)]
pub struct Get {
    pub csr: u32,
    pub oid: u32,
    pub keys: Vec<Key>,
    pub args: Vec<Value>,
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

/// Builds a from-item: a bare variable becomes a table reference, any other
/// expression a value source. (A variable without a path is assumed to be a
/// table; the grammar could disambiguate this, but this is a reasonable start.)
#[inline]
pub fn from_item(src: Expr, alias: Option<String>) -> From {
    match src {
        Expr::Var(var) => From {
            var: alias.unwrap_or_else(|| var.name.clone()),
            src: Source::Table(var.name),
            csr: None,
            oid: None,
        },
        expr => From {
            var: alias.unwrap_or_default(),
            src: Source::Value(Box::new(expr)),
            csr: None,
            oid: None,
        },
    }
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
// Parser Actions: JSONPath
//------------------------------

/// Builds a path from a root identifier and its segments.
pub fn path(identifier: String, segments: Vec<Segment>) -> Path {
    Path {
        identifier,
        segments,
    }
}

/// Builds a child segment (`.[...]`) from its selectors.
pub fn segment_child(selectors: Vec<Selector>) -> Segment {
    Segment::Child(selectors)
}

/// Builds a child name segment (`.name`).
pub fn segment_child_name(name: String) -> Segment {
    Segment::Child(vec![Selector::Name(name)])
}

/// Builds a child wildcard segment (`.*`).
pub fn segment_child_wildcard() -> Segment {
    Segment::Child(vec![Selector::Wildcard])
}

/// Builds a descendant segment (`..[...]`) from its selectors.
pub fn segment_descd(selectors: Vec<Selector>) -> Segment {
    Segment::Descd(selectors)
}

/// Builds a descendant name segment (`..name`).
pub fn segment_descd_name(name: String) -> Segment {
    Segment::Descd(vec![Selector::Name(name)])
}

/// Builds a descendant wildcard segment (`..*`).
pub fn segment_descd_wildcard() -> Segment {
    Segment::Descd(vec![Selector::Wildcard])
}

/// Builds a name selector (`name`).
pub fn selector_name(name: String) -> Selector {
    Selector::Name(name)
}

/// Builds a wildcard selector (`*`).
pub fn selector_wildcard() -> Selector {
    Selector::Wildcard
}

/// Builds an array-index selector (`[i]`).
pub fn selector_index(idx: usize) -> Selector {
    Selector::Index(idx)
}

//------------------------------
// Parser Actions: Expressions
//------------------------------

/// Builds an unbound variable reference.
#[inline]
pub fn expr_var(name: String) -> Expr {
    Expr::Var(Var::unbound(&name))
}

/// Builds a literal expression.
#[inline]
pub fn expr_lit(val: Value) -> Expr {
    Expr::Lit(val)
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
/// `not(is_null(...))`, no new IR node or opcode is needed.
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

#[cfg(test)]
mod test {
    use crate::{lexer::SqlLexer, parser::SqlParser};

    use super::*;

    fn parse(input: &str) -> Statement {
        let l = SqlLexer::new(input);
        let p = SqlParser::new();
        p.parse(l).unwrap()
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
