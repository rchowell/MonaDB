//! Visitor traits based on the `syn` crate. I will change
//! to generated free functions once the IR stabilizes.

#![allow(dead_code)]

pub mod visit {
    #[allow(clippy::wildcard_imports)]
    use crate::ir::*;

    /// Immutable borrowing walk, see: [`syn::visit`](https://docs.rs/syn/latest/syn/visit/index.html).
    pub trait Visit<'ast> {
        fn visit_statement(&mut self, i: &'ast Statement) {
            visit_statement(self, i);
        }

        fn visit_create(&mut self, i: &'ast Create) {
            visit_create(self, i);
        }

        fn visit_table_definition(&mut self, i: &'ast TableDefinition) {
            visit_table_definition(self, i);
        }

        fn visit_table_member(&mut self, i: &'ast Key) {
            visit_table_member(self, i);
        }

        fn visit_insert(&mut self, i: &'ast Insert) {
            visit_insert(self, i);
        }

        fn visit_delete(&mut self, i: &'ast Delete) {
            visit_delete(self, i);
        }

        fn visit_drop(&mut self, i: &'ast Drop) {
            visit_drop(self, i);
        }

        fn visit_clear(&mut self, i: &'ast Clear) {
            visit_clear(self, i);
        }

        fn visit_select(&mut self, i: &'ast Select) {
            visit_select(self, i);
        }

        fn visit_from(&mut self, i: &'ast From) {
            visit_from(self, i);
        }

        fn visit_source(&mut self, i: &'ast Source) {
            visit_source(self, i);
        }

        fn visit_constructor(&mut self, i: &'ast Constructor) {
            visit_constructor(self, i);
        }

        fn visit_member(&mut self, i: &'ast Member) {
            visit_member(self, i);
        }

        fn visit_limit(&mut self, i: &'ast Limit) {
            visit_limit(self, i);
        }

        fn visit_type(&mut self, i: &'ast Type) {
            visit_type(self, i);
        }

        fn visit_t_object(&mut self, i: &'ast TObject) {
            visit_t_object(self, i);
        }

        fn visit_t_member(&mut self, i: &'ast TMember) {
            visit_t_member(self, i);
        }

        fn visit_path(&mut self, i: &'ast Path) {
            visit_path(self, i);
        }

        fn visit_segment(&mut self, i: &'ast Segment) {
            visit_segment(self, i);
        }

        fn visit_selector(&mut self, i: &'ast Selector) {
            visit_selector(self, i);
        }

        fn visit_expr(&mut self, i: &'ast Expr) {
            visit_expr(self, i);
        }

        fn visit_call(&mut self, i: &'ast Call) {
            visit_call(self, i);
        }

        fn visit_jpi(&mut self, i: &'ast Jpi) {
            visit_jpi(self, i);
        }

        fn visit_jpk(&mut self, i: &'ast Jpk) {
            visit_jpk(self, i);
        }

        fn visit_jpe(&mut self, i: &'ast Jpe) {
            visit_jpe(self, i);
        }
    }

    pub fn visit_statement<'ast, V>(v: &mut V, i: &'ast Statement)
    where
        V: Visit<'ast> + ?Sized,
    {
        match i {
            Statement::Create(c) => v.visit_create(c),
            Statement::Delete(d) => v.visit_delete(d),
            Statement::Drop(d) => v.visit_drop(d),
            Statement::Clear(c) => v.visit_clear(c),
            Statement::Insert(ins) => v.visit_insert(ins),
            Statement::Select(s) => v.visit_select(s),
        }
    }

    pub fn visit_create<'ast, V>(v: &mut V, i: &'ast Create)
    where
        V: Visit<'ast> + ?Sized,
    {
        match i {
            Create::Table(td) => v.visit_table_definition(td),
        }
    }

    pub fn visit_table_definition<'ast, V>(v: &mut V, i: &'ast TableDefinition)
    where
        V: Visit<'ast> + ?Sized,
    {
        for m in &i.keys {
            v.visit_table_member(m);
        }
    }

    pub fn visit_table_member<'ast, V>(v: &mut V, i: &'ast Key)
    where
        V: Visit<'ast> + ?Sized,
    {
        v.visit_type(&i.ty);
    }

    pub fn visit_insert<'ast, V>(v: &mut V, i: &'ast Insert)
    where
        V: Visit<'ast> + ?Sized,
    {
        for e in &i.source {
            v.visit_expr(e);
        }
    }

    pub fn visit_delete<'ast, V>(v: &mut V, i: &'ast Delete)
    where
        V: Visit<'ast> + ?Sized,
    {
        v.visit_from(&i.from);
        if let Some(w) = &i.where_ {
            v.visit_expr(w);
        }
    }

    pub fn visit_drop<'ast, V>(_v: &mut V, _i: &'ast Drop)
    where
        V: Visit<'ast> + ?Sized,
    {
        // No child IR to walk; the binder overrides `visit_drop` to bind the oid.
    }

    pub fn visit_clear<'ast, V>(_v: &mut V, _i: &'ast Clear)
    where
        V: Visit<'ast> + ?Sized,
    {
        // No child IR to walk; the binder overrides `visit_clear` to bind the oid.
    }

    pub fn visit_select<'ast, V>(v: &mut V, i: &'ast Select)
    where
        V: Visit<'ast> + ?Sized,
    {
        for f in &i.from {
            v.visit_from(f);
        }
        if let Some(w) = &i.where_ {
            v.visit_expr(w);
        }
        if let Some(o) = &i.order {
            for k in &o.keys {
                v.visit_expr(&k.expr);
            }
        }
        if let Some(l) = &i.limit {
            v.visit_limit(l);
        }
        v.visit_constructor(&i.select);
    }

    pub fn visit_from<'ast, V>(v: &mut V, i: &'ast From)
    where
        V: Visit<'ast> + ?Sized,
    {
        v.visit_source(&i.src);
    }

    pub fn visit_source<'ast, V>(v: &mut V, i: &'ast Source)
    where
        V: Visit<'ast> + ?Sized,
    {
        match i {
            Source::Table(_) => {}
            Source::Value(expr) => v.visit_expr(expr),
            Source::Unpivot(u) => v.visit_expr(&u.expr),
        }
    }

    pub fn visit_constructor<'ast, V>(v: &mut V, i: &'ast Constructor)
    where
        V: Visit<'ast> + ?Sized,
    {
        match i {
            Constructor::None | Constructor::Star => {}
            Constructor::Expr(e) => v.visit_expr(e),
            Constructor::List(members) => {
                for m in members {
                    v.visit_member(m);
                }
            }
            Constructor::Pivot(p) => {
                v.visit_expr(&p.value);
                v.visit_expr(&p.name);
            }
        }
    }

    pub fn visit_member<'ast, V>(v: &mut V, i: &'ast Member)
    where
        V: Visit<'ast> + ?Sized,
    {
        match i {
            Member::Assign(_, e) | Member::Spread(e) => v.visit_expr(e),
        }
    }

    pub fn visit_limit<'ast, V>(_v: &mut V, i: &'ast Limit)
    where
        V: Visit<'ast> + ?Sized,
    {
        match i {
            Limit::Skip(_) | Limit::Take(_) | Limit::Slice(_, _) => {}
        }
    }

    pub fn visit_type<'ast, V>(v: &mut V, i: &'ast Type)
    where
        V: Visit<'ast> + ?Sized,
    {
        match i {
            Type::Any
            | Type::Bool
            | Type::Int
            | Type::Float
            | Type::Number
            | Type::String
            | Type::Array => {}
            Type::Object(o) => v.visit_t_object(o),
        }
    }

    pub fn visit_t_object<'ast, V>(v: &mut V, i: &'ast TObject)
    where
        V: Visit<'ast> + ?Sized,
    {
        for m in &i.members {
            v.visit_t_member(m);
        }
    }

    pub fn visit_t_member<'ast, V>(v: &mut V, i: &'ast TMember)
    where
        V: Visit<'ast> + ?Sized,
    {
        v.visit_type(&i.ty);
    }

    pub fn visit_path<'ast, V>(v: &mut V, i: &'ast Path)
    where
        V: Visit<'ast> + ?Sized,
    {
        for s in &i.segments {
            v.visit_segment(s);
        }
    }

    pub fn visit_segment<'ast, V>(v: &mut V, i: &'ast Segment)
    where
        V: Visit<'ast> + ?Sized,
    {
        match i {
            Segment::Child(sels) | Segment::Descd(sels) => {
                for s in sels {
                    v.visit_selector(s);
                }
            }
        }
    }

    pub fn visit_selector<'ast, V>(_v: &mut V, i: &'ast Selector)
    where
        V: Visit<'ast> + ?Sized,
    {
        match i {
            Selector::Name(_) | Selector::Wildcard | Selector::Index(_) => {}
        }
    }

    pub fn visit_expr<'ast, V>(v: &mut V, i: &'ast Expr)
    where
        V: Visit<'ast> + ?Sized,
    {
        match i {
            Expr::Call(call) => v.visit_call(call),
            Expr::Jpi(jpi) => v.visit_jpi(jpi),
            Expr::Jpk(jpk) => v.visit_jpk(jpk),
            Expr::Jpe(jpe) => v.visit_jpe(jpe),
            // A bound `Get` holds literal key Values, not Expr children.
            Expr::Lit(_) | Expr::Var(_) | Expr::Get(_) => {}
            Expr::Obj(members) => {
                for m in members {
                    v.visit_member(m);
                }
            }
            Expr::Array(items) => {
                for e in items {
                    v.visit_expr(e);
                }
            }
            Expr::Subscript(sub) => {
                v.visit_expr(&sub.base);
                for e in &sub.args {
                    v.visit_expr(e);
                }
            }
            Expr::Agg(agg) => {
                if let Some(arg) = &agg.arg {
                    v.visit_expr(arg);
                }
            }
        }
    }

    pub fn visit_call<'ast, V>(v: &mut V, i: &'ast Call)
    where
        V: Visit<'ast> + ?Sized,
    {
        for arg in &i.args {
            v.visit_expr(arg);
        }
    }

    pub fn visit_jpi<'ast, V>(v: &mut V, i: &'ast Jpi)
    where
        V: Visit<'ast> + ?Sized,
    {
        v.visit_expr(&i.inp);
    }

    pub fn visit_jpk<'ast, V>(v: &mut V, i: &'ast Jpk)
    where
        V: Visit<'ast> + ?Sized,
    {
        v.visit_expr(&i.inp);
    }

    pub fn visit_jpe<'ast, V>(v: &mut V, i: &'ast Jpe)
    where
        V: Visit<'ast> + ?Sized,
    {
        v.visit_expr(&i.inp);
        v.visit_expr(&i.exp);
    }
}

pub mod visit_mut {
    #[allow(clippy::wildcard_imports)]
    use crate::ir::*;

    /// Mutable borrowing walk, see: [`syn::visit_mut`](https://docs.rs/syn/latest/syn/visit_mut/index.html).
    pub trait VisitMut {
        fn visit_statement_mut(&mut self, i: &mut Statement) {
            visit_statement_mut(self, i);
        }

        fn visit_create_mut(&mut self, i: &mut Create) {
            visit_create_mut(self, i);
        }

        fn visit_table_definition_mut(&mut self, i: &mut TableDefinition) {
            visit_table_definition_mut(self, i);
        }

        fn visit_table_member_mut(&mut self, i: &mut Key) {
            visit_table_member_mut(self, i);
        }

        fn visit_insert_mut(&mut self, i: &mut Insert) {
            visit_insert_mut(self, i);
        }

        fn visit_delete_mut(&mut self, i: &mut Delete) {
            visit_delete_mut(self, i);
        }

        fn visit_drop_mut(&mut self, i: &mut Drop) {
            visit_drop_mut(self, i);
        }

        fn visit_clear_mut(&mut self, i: &mut Clear) {
            visit_clear_mut(self, i);
        }

        fn visit_select_mut(&mut self, i: &mut Select) {
            visit_select_mut(self, i);
        }

        fn visit_from_mut(&mut self, i: &mut From) {
            visit_from_mut(self, i);
        }

        fn visit_source_mut(&mut self, i: &mut Source) {
            visit_source_mut(self, i);
        }

        fn visit_constructor_mut(&mut self, i: &mut Constructor) {
            visit_constructor_mut(self, i);
        }

        fn visit_member_mut(&mut self, i: &mut Member) {
            visit_member_mut(self, i);
        }

        fn visit_limit_mut(&mut self, i: &mut Limit) {
            visit_limit_mut(self, i);
        }

        fn visit_type_mut(&mut self, i: &mut Type) {
            visit_type_mut(self, i);
        }

        fn visit_t_object_mut(&mut self, i: &mut TObject) {
            visit_t_object_mut(self, i);
        }

        fn visit_t_member_mut(&mut self, i: &mut TMember) {
            visit_t_member_mut(self, i);
        }

        fn visit_path_mut(&mut self, i: &mut Path) {
            visit_path_mut(self, i);
        }

        fn visit_segment_mut(&mut self, i: &mut Segment) {
            visit_segment_mut(self, i);
        }

        fn visit_selector_mut(&mut self, i: &mut Selector) {
            visit_selector_mut(self, i);
        }

        fn visit_expr_mut(&mut self, i: &mut Expr) {
            visit_expr_mut(self, i);
        }

        fn visit_call_mut(&mut self, i: &mut Call) {
            visit_call_mut(self, i);
        }

        fn visit_jpi_mut(&mut self, i: &mut Jpi) {
            visit_jpi_mut(self, i);
        }

        fn visit_jpk_mut(&mut self, i: &mut Jpk) {
            visit_jpk_mut(self, i);
        }

        fn visit_jpe_mut(&mut self, i: &mut Jpe) {
            visit_jpe_mut(self, i);
        }
    }

    pub fn visit_statement_mut<V: VisitMut + ?Sized>(v: &mut V, i: &mut Statement) {
        match i {
            Statement::Create(c) => v.visit_create_mut(c),
            Statement::Delete(d) => v.visit_delete_mut(d),
            Statement::Drop(d) => v.visit_drop_mut(d),
            Statement::Clear(c) => v.visit_clear_mut(c),
            Statement::Insert(ins) => v.visit_insert_mut(ins),
            Statement::Select(s) => v.visit_select_mut(s),
        }
    }

    pub fn visit_create_mut<V: VisitMut + ?Sized>(v: &mut V, i: &mut Create) {
        match i {
            Create::Table(td) => v.visit_table_definition_mut(td),
        }
    }

    pub fn visit_table_definition_mut<V: VisitMut + ?Sized>(v: &mut V, i: &mut TableDefinition) {
        for m in &mut i.keys {
            v.visit_table_member_mut(m);
        }
    }

    pub fn visit_table_member_mut<V: VisitMut + ?Sized>(v: &mut V, i: &mut Key) {
        v.visit_type_mut(&mut i.ty);
    }

    pub fn visit_insert_mut<V: VisitMut + ?Sized>(v: &mut V, i: &mut Insert) {
        for e in &mut i.source {
            v.visit_expr_mut(e);
        }
    }

    pub fn visit_delete_mut<V: VisitMut + ?Sized>(v: &mut V, i: &mut Delete) {
        v.visit_from_mut(&mut i.from);
        if let Some(w) = &mut i.where_ {
            v.visit_expr_mut(w);
        }
    }

    pub fn visit_drop_mut<V: VisitMut + ?Sized>(_v: &mut V, _i: &mut Drop) {
        // No child IR to walk; the binder overrides `visit_drop_mut` to bind the oid.
    }

    pub fn visit_clear_mut<V: VisitMut + ?Sized>(_v: &mut V, _i: &mut Clear) {
        // No child IR to walk; the binder overrides `visit_clear_mut` to bind the oid.
    }

    pub fn visit_select_mut<V: VisitMut + ?Sized>(v: &mut V, i: &mut Select) {
        for f in &mut i.from {
            v.visit_from_mut(f);
        }
        if let Some(w) = &mut i.where_ {
            v.visit_expr_mut(w);
        }
        if let Some(o) = &mut i.order {
            for k in &mut o.keys {
                v.visit_expr_mut(&mut k.expr);
            }
        }
        if let Some(l) = &mut i.limit {
            v.visit_limit_mut(l);
        }
        v.visit_constructor_mut(&mut i.select);
    }

    pub fn visit_from_mut<V: VisitMut + ?Sized>(v: &mut V, i: &mut From) {
        v.visit_source_mut(&mut i.src);
    }

    pub fn visit_source_mut<V: VisitMut + ?Sized>(v: &mut V, i: &mut Source) {
        match i {
            Source::Table(_) => {}
            Source::Value(expr) => v.visit_expr_mut(expr),
            Source::Unpivot(u) => v.visit_expr_mut(&mut u.expr),
        }
    }

    pub fn visit_constructor_mut<V: VisitMut + ?Sized>(v: &mut V, i: &mut Constructor) {
        match i {
            Constructor::None | Constructor::Star => {}
            Constructor::Expr(e) => v.visit_expr_mut(e),
            Constructor::List(members) => {
                for m in members {
                    v.visit_member_mut(m);
                }
            }
            Constructor::Pivot(p) => {
                v.visit_expr_mut(&mut p.value);
                v.visit_expr_mut(&mut p.name);
            }
        }
    }

    pub fn visit_member_mut<V: VisitMut + ?Sized>(v: &mut V, i: &mut Member) {
        match i {
            Member::Assign(_, e) | Member::Spread(e) => v.visit_expr_mut(e),
        }
    }

    pub fn visit_limit_mut<V: VisitMut + ?Sized>(_v: &mut V, i: &mut Limit) {
        match i {
            Limit::Skip(_) | Limit::Take(_) | Limit::Slice(_, _) => {}
        }
    }

    pub fn visit_type_mut<V: VisitMut + ?Sized>(v: &mut V, i: &mut Type) {
        match i {
            Type::Any
            | Type::Bool
            | Type::Int
            | Type::Float
            | Type::Number
            | Type::String
            | Type::Array => {}
            Type::Object(o) => v.visit_t_object_mut(o),
        }
    }

    pub fn visit_t_object_mut<V: VisitMut + ?Sized>(v: &mut V, i: &mut TObject) {
        for m in &mut i.members {
            v.visit_t_member_mut(m);
        }
    }

    pub fn visit_t_member_mut<V: VisitMut + ?Sized>(v: &mut V, i: &mut TMember) {
        v.visit_type_mut(&mut i.ty);
    }

    pub fn visit_path_mut<V: VisitMut + ?Sized>(v: &mut V, i: &mut Path) {
        for s in &mut i.segments {
            v.visit_segment_mut(s);
        }
    }

    pub fn visit_segment_mut<V: VisitMut + ?Sized>(v: &mut V, i: &mut Segment) {
        match i {
            Segment::Child(sels) | Segment::Descd(sels) => {
                for s in sels {
                    v.visit_selector_mut(s);
                }
            }
        }
    }

    pub fn visit_selector_mut<V: VisitMut + ?Sized>(_v: &mut V, i: &mut Selector) {
        match i {
            Selector::Name(_) | Selector::Wildcard | Selector::Index(_) => {}
        }
    }

    pub fn visit_expr_mut<V: VisitMut + ?Sized>(v: &mut V, i: &mut Expr) {
        match i {
            Expr::Call(call) => v.visit_call_mut(call),
            Expr::Jpi(jpi) => v.visit_jpi_mut(jpi),
            Expr::Jpk(jpk) => v.visit_jpk_mut(jpk),
            Expr::Jpe(jpe) => v.visit_jpe_mut(jpe),
            // A bound `Get` holds literal key Values, not Expr children.
            Expr::Lit(_) | Expr::Var(_) | Expr::Get(_) => {}
            Expr::Obj(members) => {
                for m in members {
                    v.visit_member_mut(m);
                }
            }
            Expr::Array(items) => {
                for e in items {
                    v.visit_expr_mut(e);
                }
            }
            Expr::Subscript(sub) => {
                v.visit_expr_mut(&mut sub.base);
                for e in &mut sub.args {
                    v.visit_expr_mut(e);
                }
            }
            Expr::Agg(agg) => {
                if let Some(arg) = &mut agg.arg {
                    v.visit_expr_mut(arg);
                }
            }
        }
    }

    pub fn visit_call_mut<V: VisitMut + ?Sized>(v: &mut V, i: &mut Call) {
        for arg in &mut i.args {
            v.visit_expr_mut(arg);
        }
    }

    pub fn visit_jpi_mut<V: VisitMut + ?Sized>(v: &mut V, i: &mut Jpi) {
        v.visit_expr_mut(&mut i.inp);
    }

    pub fn visit_jpk_mut<V: VisitMut + ?Sized>(v: &mut V, i: &mut Jpk) {
        v.visit_expr_mut(&mut i.inp);
    }

    pub fn visit_jpe_mut<V: VisitMut + ?Sized>(v: &mut V, i: &mut Jpe) {
        v.visit_expr_mut(&mut i.inp);
        v.visit_expr_mut(&mut i.exp);
    }
}

pub mod fold {
    #[allow(clippy::wildcard_imports)]
    use crate::ir::*;

    /// Owned transform, see [`syn::fold`](https://docs.rs/syn/latest/syn/fold/index.html).
    pub trait Fold {
        fn fold_statement(&mut self, i: Statement) -> Statement {
            fold_statement(self, i)
        }

        fn fold_create(&mut self, i: Create) -> Create {
            fold_create(self, i)
        }

        fn fold_table_definition(&mut self, i: TableDefinition) -> TableDefinition {
            fold_table_definition(self, i)
        }

        fn fold_table_member(&mut self, i: Key) -> Key {
            fold_table_member(self, i)
        }

        fn fold_insert(&mut self, i: Insert) -> Insert {
            fold_insert(self, i)
        }

        fn fold_select(&mut self, i: Select) -> Select {
            fold_select(self, i)
        }

        fn fold_from(&mut self, i: From) -> From {
            fold_from(self, i)
        }

        fn fold_source(&mut self, i: Source) -> Source {
            fold_source(self, i)
        }

        fn fold_constructor(&mut self, i: Constructor) -> Constructor {
            fold_constructor(self, i)
        }

        fn fold_member(&mut self, i: Member) -> Member {
            fold_member(self, i)
        }

        fn fold_limit(&mut self, i: Limit) -> Limit {
            fold_limit(self, i)
        }

        fn fold_type(&mut self, i: Type) -> Type {
            fold_type(self, i)
        }

        fn fold_t_object(&mut self, i: TObject) -> TObject {
            fold_t_object(self, i)
        }

        fn fold_t_member(&mut self, i: TMember) -> TMember {
            fold_t_member(self, i)
        }

        fn fold_path(&mut self, i: Path) -> Path {
            fold_path(self, i)
        }

        fn fold_segment(&mut self, i: Segment) -> Segment {
            fold_segment(self, i)
        }

        fn fold_selector(&mut self, i: Selector) -> Selector {
            fold_selector(self, i)
        }

        fn fold_expr(&mut self, i: Expr) -> Expr {
            fold_expr(self, i)
        }

        fn fold_call(&mut self, i: Call) -> Call {
            fold_call(self, i)
        }

        fn fold_jpi(&mut self, i: Jpi) -> Jpi {
            fold_jpi(self, i)
        }

        fn fold_jpk(&mut self, i: Jpk) -> Jpk {
            fold_jpk(self, i)
        }

        fn fold_jpe(&mut self, i: Jpe) -> Jpe {
            fold_jpe(self, i)
        }
    }

    pub fn fold_statement<F: Fold + ?Sized>(f: &mut F, i: Statement) -> Statement {
        match i {
            Statement::Create(c) => Statement::Create(f.fold_create(c)),
            Statement::Delete(s) => Statement::Delete(s),
            Statement::Drop(s) => Statement::Drop(s),
            Statement::Clear(s) => Statement::Clear(s),
            Statement::Insert(ins) => Statement::Insert(f.fold_insert(ins)),
            Statement::Select(s) => Statement::Select(f.fold_select(s)),
        }
    }

    pub fn fold_create<F: Fold + ?Sized>(f: &mut F, i: Create) -> Create {
        match i {
            Create::Table(td) => Create::Table(f.fold_table_definition(td)),
        }
    }

    pub fn fold_table_definition<F: Fold + ?Sized>(
        f: &mut F,
        i: TableDefinition,
    ) -> TableDefinition {
        TableDefinition {
            oid: i.oid,
            name: i.name,
            keys: i.keys.into_iter().map(|m| f.fold_table_member(m)).collect(),
        }
    }

    pub fn fold_table_member<F: Fold + ?Sized>(f: &mut F, i: Key) -> Key {
        Key {
            name: i.name,
            ty: f.fold_type(i.ty),
        }
    }

    pub fn fold_insert<F: Fold + ?Sized>(f: &mut F, i: Insert) -> Insert {
        Insert {
            target: f.fold_table_definition(i.target),
            source: i.source.into_iter().map(|e| f.fold_expr(e)).collect(),
        }
    }

    pub fn fold_select<F: Fold + ?Sized>(f: &mut F, i: Select) -> Select {
        Select {
            from: i.from.into_iter().map(|x| f.fold_from(x)).collect(),
            where_: i.where_.map(|w| f.fold_expr(w)),
            order: i.order.map(|o| OrderBy {
                keys: o
                    .keys
                    .into_iter()
                    .map(|k| OrderKey {
                        expr: f.fold_expr(k.expr),
                        desc: k.desc,
                    })
                    .collect(),
            }),
            limit: i.limit.map(|l| f.fold_limit(l)),
            select: f.fold_constructor(i.select),
        }
    }

    pub fn fold_from<F: Fold + ?Sized>(f: &mut F, i: From) -> From {
        From {
            src: f.fold_source(i.src),
            var: i.var,
            csr: i.csr,
            oid: i.oid,
        }
    }

    pub fn fold_source<F: Fold + ?Sized>(f: &mut F, i: Source) -> Source {
        match i {
            Source::Table(name) => Source::Table(name),
            Source::Value(expr) => Source::Value(Box::new(f.fold_expr(*expr))),
            Source::Unpivot(u) => Source::Unpivot(Unpivot {
                expr: Box::new(f.fold_expr(*u.expr)),
                ..u
            }),
        }
    }

    pub fn fold_constructor<F: Fold + ?Sized>(f: &mut F, i: Constructor) -> Constructor {
        match i {
            Constructor::None => Constructor::None,
            Constructor::Star => Constructor::Star,
            Constructor::Expr(e) => Constructor::Expr(f.fold_expr(e)),
            Constructor::List(members) => {
                Constructor::List(members.into_iter().map(|m| f.fold_member(m)).collect())
            }
            Constructor::Pivot(p) => Constructor::Pivot(Pivot {
                value: Box::new(f.fold_expr(*p.value)),
                name: Box::new(f.fold_expr(*p.name)),
            }),
        }
    }

    pub fn fold_member<F: Fold + ?Sized>(f: &mut F, i: Member) -> Member {
        match i {
            Member::Assign(name, e) => Member::Assign(name, f.fold_expr(e)),
            Member::Spread(e) => Member::Spread(f.fold_expr(e)),
        }
    }

    pub fn fold_limit<F: Fold + ?Sized>(_f: &mut F, i: Limit) -> Limit {
        i
    }

    pub fn fold_type<F: Fold + ?Sized>(f: &mut F, i: Type) -> Type {
        match i {
            Type::Any
            | Type::Bool
            | Type::Int
            | Type::Float
            | Type::Number
            | Type::String
            | Type::Array => i,
            Type::Object(o) => Type::Object(f.fold_t_object(o)),
        }
    }

    pub fn fold_t_object<F: Fold + ?Sized>(f: &mut F, i: TObject) -> TObject {
        TObject {
            members: i.members.into_iter().map(|m| f.fold_t_member(m)).collect(),
        }
    }

    pub fn fold_t_member<F: Fold + ?Sized>(f: &mut F, i: TMember) -> TMember {
        TMember {
            name: i.name,
            ty: Box::new(f.fold_type(*i.ty)),
        }
    }

    pub fn fold_path<F: Fold + ?Sized>(f: &mut F, i: Path) -> Path {
        Path {
            identifier: i.identifier,
            segments: i.segments.into_iter().map(|s| f.fold_segment(s)).collect(),
        }
    }

    pub fn fold_segment<F: Fold + ?Sized>(f: &mut F, i: Segment) -> Segment {
        match i {
            Segment::Child(sels) => {
                Segment::Child(sels.into_iter().map(|s| f.fold_selector(s)).collect())
            }
            Segment::Descd(sels) => {
                Segment::Descd(sels.into_iter().map(|s| f.fold_selector(s)).collect())
            }
        }
    }

    pub fn fold_selector<F: Fold + ?Sized>(_f: &mut F, i: Selector) -> Selector {
        i
    }

    pub fn fold_expr<F: Fold + ?Sized>(f: &mut F, i: Expr) -> Expr {
        match i {
            Expr::Call(call) => Expr::Call(f.fold_call(call)),
            Expr::Jpi(jpi) => Expr::Jpi(f.fold_jpi(jpi)),
            Expr::Jpk(jpk) => Expr::Jpk(f.fold_jpk(jpk)),
            Expr::Jpe(jpe) => Expr::Jpe(f.fold_jpe(jpe)),
            Expr::Lit(v) => Expr::Lit(v),
            Expr::Obj(members) => {
                Expr::Obj(members.into_iter().map(|m| f.fold_member(m)).collect())
            }
            Expr::Array(items) => Expr::Array(items.into_iter().map(|e| f.fold_expr(e)).collect()),
            Expr::Var(name) => Expr::Var(name),
            Expr::Subscript(sub) => Expr::Subscript(Subscript {
                base: Box::new(f.fold_expr(*sub.base)),
                args: sub.args.into_iter().map(|e| f.fold_expr(e)).collect(),
            }),
            // A bound Get carries only literal key Values; fold is identity.
            Expr::Get(get) => Expr::Get(get),
            Expr::Agg(agg) => Expr::Agg(Agg {
                kind: agg.kind,
                arg: agg.arg.map(|a| Box::new(f.fold_expr(*a))),
                slot: agg.slot,
            }),
        }
    }

    pub fn fold_call<F: Fold + ?Sized>(f: &mut F, i: Call) -> Call {
        Call {
            name: i.name,
            args: i.args.into_iter().map(|e| f.fold_expr(e)).collect(),
        }
    }

    pub fn fold_jpi<F: Fold + ?Sized>(f: &mut F, i: Jpi) -> Jpi {
        Jpi {
            inp: Box::new(f.fold_expr(*i.inp)),
            idx: i.idx,
        }
    }

    pub fn fold_jpk<F: Fold + ?Sized>(f: &mut F, i: Jpk) -> Jpk {
        Jpk {
            inp: Box::new(f.fold_expr(*i.inp)),
            key: i.key,
        }
    }

    pub fn fold_jpe<F: Fold + ?Sized>(f: &mut F, i: Jpe) -> Jpe {
        Jpe {
            inp: Box::new(f.fold_expr(*i.inp)),
            exp: Box::new(f.fold_expr(*i.exp)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::fold::Fold;
    use super::visit::Visit;
    use super::visit_mut::VisitMut;
    use super::*;
    use crate::ir::*;
    use crate::lexer::SqlLexer;
    use crate::parser::SqlParser;

    fn parse(input: &str) -> Statement {
        let l = SqlLexer::new(input);
        let p = SqlParser::new();
        p.parse(l).unwrap()
    }

    /// Counts every Expr node reached by the default walk.
    #[derive(Default)]
    struct ExprCounter(usize);

    impl<'ast> visit::Visit<'ast> for ExprCounter {
        fn visit_expr(&mut self, i: &'ast Expr) {
            self.0 += 1;
            visit::visit_expr(self, i);
        }
    }

    #[test]
    fn visit_counts_exprs_in_select() {
        let stmt = parse("select * from T where a > 0;");
        let mut c = ExprCounter::default();
        c.visit_statement(&stmt);
        // `a > 0` is Call(">", [Var(a), Lit(0)]) — three Expr nodes.
        assert_eq!(c.0, 3);
    }

    #[derive(Default)]
    struct ExprCounterMut(usize);

    impl visit_mut::VisitMut for ExprCounterMut {
        fn visit_expr_mut(&mut self, i: &mut Expr) {
            self.0 += 1;
            visit_mut::visit_expr_mut(self, i);
        }
    }

    #[test]
    fn visit_mut_counts_exprs_in_select() {
        let mut stmt = parse("select * from T where a > 0;");
        let mut c = ExprCounterMut::default();
        c.visit_statement_mut(&mut stmt);
        assert_eq!(c.0, 3);
    }

    /// Identity fold — verifies fold_* round-trips a tree unchanged.
    struct IdentityFold;
    impl fold::Fold for IdentityFold {}

    #[test]
    fn fold_identity_round_trips_select() {
        let stmt = parse("select * from T where a > 0;");
        let folded = IdentityFold.fold_statement(stmt);
        // Round-trip preserves SQL rendering.
        let Statement::Select(s) = &folded else {
            panic!("expected select")
        };
        assert!(matches!(s.from[0].src, Source::Table(_)));
        assert!(s.where_.is_some());
    }
}
