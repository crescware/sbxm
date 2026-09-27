//! OS層が分岐を持たないことの検査。
//!
//! OS層はcoverageの母集団から外す。外してよいのは、判断を置けない形に限るからである。関数の
//! 本体は1つの呼び出し式か構造体の値とし、`?`だけを許す。`if`・`match`・`loop`・`while`・`for`に加え、
//! `let`、closure、macro、`&&`と`||`、method呼び出しの連鎖、分岐を包んだcombinator
//! （`map`、`then`、`unwrap_or_else`など）も置けない。どれもkeywordではなくても、llvm-covが
//! 数える分岐か、分岐を呼び出し先へ隠すものである。
//!
//! itemは、`use`、構造体、`static`・`const`、`impl`、関数、`#[cfg(test)]`で契約testを指す
//! module宣言だけとする。`mod.rs`だけは、OS層の各fileを指すmodule宣言も置ける。指した先の
//! fileも同じ検査を受ける。中身を持つmoduleはどこにも置けない。列挙と`match`は対で判断を作り、
//! traitは判断側が持つ。

use syn::visit::Visit;
use syn::{Expr, ImplItem, Item, Stmt};

/// 分岐を呼び出し先へ隠すmethod。受け手の型は見ない。
const COMBINATORS: [&str; 41] = [
    "then",
    "then_some",
    "map",
    "map_err",
    "map_or",
    "map_or_else",
    "and_then",
    "or_else",
    "or",
    "and",
    "xor",
    "unwrap",
    "expect",
    "unwrap_or",
    "unwrap_or_else",
    "unwrap_or_default",
    "ok",
    "err",
    "ok_or",
    "ok_or_else",
    "filter",
    "filter_map",
    "flatten",
    "is_some_and",
    "is_none_or",
    "is_ok_and",
    "is_err_and",
    "iter",
    "iter_mut",
    "into_iter",
    "zip",
    "fold",
    "any",
    "all",
    "find",
    "find_map",
    "position",
    "take_while",
    "skip_while",
    "inspect",
    "get_or_insert_with",
];

/// OS層に置いた1 fileの分岐と、置けないitem。`entry`はそのfileが`mod.rs`であること。
pub fn violations(text: &str, entry: bool) -> Result<Vec<String>, syn::Error> {
    let file = syn::parse_file(text)?;
    let mut found = Vec::new();
    for item in &file.items {
        match item {
            Item::Use(_) | Item::Struct(_) => {}
            Item::Mod(module)
                if module.content.is_none() && (entry || only_for_tests(&module.attrs)) => {}
            Item::Static(item) => expression(&item.ident.to_string(), &item.expr, &mut found),
            Item::Const(item) => expression(&item.ident.to_string(), &item.expr, &mut found),
            Item::Fn(function) => {
                body(&function.sig.ident.to_string(), &function.block, &mut found);
            }
            Item::Impl(block) => {
                for item in &block.items {
                    match item {
                        ImplItem::Fn(function) => {
                            body(&function.sig.ident.to_string(), &function.block, &mut found);
                        }
                        ImplItem::Const(item) => {
                            expression(&item.ident.to_string(), &item.expr, &mut found);
                        }
                        ImplItem::Type(_) => {}
                        _ => found.push("an impl holds something other than functions".to_string()),
                    }
                }
            }
            _ => found.push(format!("{} does not belong in the OS layer", kind(item))),
        }
    }
    Ok(found)
}

/// `#[cfg(test)]`が付いているか。契約testを指すmodule宣言だけが、OS層に置ける。
fn only_for_tests(attributes: &[syn::Attribute]) -> bool {
    attributes.iter().any(|attribute| {
        attribute.path().is_ident("cfg")
            && attribute
                .parse_args::<syn::Path>()
                .is_ok_and(|predicate| predicate.is_ident("test"))
    })
}

fn kind(item: &Item) -> &'static str {
    match item {
        Item::Enum(_) => "an enum",
        Item::Trait(_) => "a trait",
        Item::Macro(_) => "a macro",
        Item::Mod(module) if module.content.is_some() => "an inline module",
        Item::Mod(_) => "a module that is not a contract test",
        Item::ForeignMod(_) => "an extern block",
        _ => "an item of this kind",
    }
}

/// 関数の本体が1つの呼び出し式であり、分岐を持たないか。
fn body(name: &str, block: &syn::Block, found: &mut Vec<String>) {
    let [statement] = block.stmts.as_slice() else {
        found.push(format!(
            "fn {name}: the body holds {} statements, not one call",
            block.stmts.len()
        ));
        return;
    };
    let Stmt::Expr(expr, _) = statement else {
        found.push(format!("fn {name}: the body is not one call expression"));
        return;
    };
    if !is_call(expr) {
        found.push(format!("fn {name}: the body is not one call expression"));
    }
    expression(name, expr, found);
}

/// `?`を外すと、関数やmethodの呼び出しか、構造体の値か。構造体の各fieldも分岐を持てない。
fn is_call(expr: &Expr) -> bool {
    match expr {
        Expr::Try(inner) => is_call(&inner.expr),
        Expr::Paren(inner) => is_call(&inner.expr),
        Expr::Call(_) | Expr::MethodCall(_) | Expr::Struct(_) => true,
        _ => false,
    }
}

/// 式のどこにも分岐が無いか。
fn expression(name: &str, expr: &Expr, found: &mut Vec<String>) {
    let mut branches = Branches {
        name,
        found: Vec::new(),
    };
    branches.visit_expr(expr);
    found.append(&mut branches.found);
}

struct Branches<'a> {
    name: &'a str,
    found: Vec<String>,
}

impl Branches<'_> {
    fn report(&mut self, what: &str) {
        self.found
            .push(format!("{}: {what} is a branch", self.name));
    }
}

impl<'ast> Visit<'ast> for Branches<'_> {
    fn visit_expr(&mut self, expr: &'ast Expr) {
        match expr {
            Expr::If(_) => self.report("if"),
            Expr::Match(_) => self.report("match"),
            Expr::Loop(_) => self.report("loop"),
            Expr::While(_) => self.report("while"),
            Expr::ForLoop(_) => self.report("for"),
            Expr::Let(_) => self.report("let"),
            Expr::Closure(_) => self.report("a closure"),
            Expr::Macro(_) => self.report("a macro"),
            Expr::Block(_) | Expr::Unsafe(_) | Expr::Async(_) | Expr::Const(_) => {
                self.report("a block");
            }
            Expr::Return(_) | Expr::Break(_) | Expr::Continue(_) => {
                self.report("an early exit");
            }
            Expr::Binary(binary) if matches!(binary.op, syn::BinOp::And(_) | syn::BinOp::Or(_)) => {
                self.report("a short-circuit operator");
            }
            Expr::MethodCall(call) => {
                let method = call.method.to_string();
                if COMBINATORS.contains(&method.as_str()) {
                    self.report(&format!("`.{method}()`"));
                }
                if matches!(*call.receiver, Expr::MethodCall(_)) {
                    self.report(&format!("a chain ending in `.{method}()`"));
                }
            }
            _ => {}
        }
        syn::visit::visit_expr(self, expr);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_single_call_is_accepted() -> Result<(), syn::Error> {
        let text = "
            use a::B;
            static START: Lazy<T> = Lazy::new(T::read);
            pub struct Thing;
            impl Port for Thing {
                fn read(&self) -> u32 { START.value() }
                fn write(&self, value: u32) { target::write(value); }
                fn open(&self, path: &P) -> R<F> { Ok(target::open(path, A | B)?) }
            }
            #[cfg(test)]
            #[path = \"thing_test.rs\"]
            mod thing_test;
        ";
        assert_eq!(violations(text, false)?, Vec::<String>::new());
        Ok(())
    }

    #[test]
    fn control_flow_and_hidden_branches_are_reported() -> Result<(), syn::Error> {
        for body in [
            "if a { b() } else { c() }",
            "match a { _ => b() }",
            "f(|x| x)",
            "f(a && b())",
            "f(a || b())",
            "value.map(g)",
            "flag.then(g)",
            "f(g().unwrap_or_default())",
            "f(a.b().c())",
            "f(matches!(a, B))",
            "f({ g() })",
        ] {
            let text = format!("fn f() {{ {body} }}");
            assert!(!violations(&text, false)?.is_empty(), "{body}");
        }
        Ok(())
    }

    #[test]
    fn more_than_one_call_is_reported() -> Result<(), syn::Error> {
        for body in ["a(); b()", "let x = a(); b(x)", "x", "1 + 2"] {
            let text = format!("fn f() {{ {body} }}");
            assert!(!violations(&text, false)?.is_empty(), "{body}");
        }
        Ok(())
    }

    #[test]
    fn items_that_carry_decisions_are_reported() -> Result<(), syn::Error> {
        for text in [
            "enum Choice { A, B }",
            "trait Port { fn read(&self) -> u32; }",
            "macro_rules! m { () => {} }",
            "mod inner { fn f() {} }",
            "mod helper;",
        ] {
            assert_eq!(violations(text, false)?.len(), 1, "{text}");
        }
        // 入口は、OS層の各fileを指せる。中身を持つmoduleは入口にも置けない。
        assert!(violations("mod system_clock;", true)?.is_empty());
        assert_eq!(violations("mod inner { fn f() {} }", true)?.len(), 1);
        Ok(())
    }
}
