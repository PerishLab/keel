use crate::head::{place, put};
use crate::*;
use syn::parse::Parser;
use syn::punctuated::Punctuated;
use syn::{Attribute, Expr, ExprAssign, Ident, Meta, Token};

pub(crate) trait Spell {
    fn ident(&self) -> syn::Result<Ident>;
    fn head(&self) -> syn::Result<Ident>;
    fn slot(&self) -> syn::Result<(String, Ident)>;
}

impl Spell for Expr {
    fn ident(&self) -> syn::Result<Ident> {
        match self {
            Expr::Path(path) => path
                .path
                .get_ident()
                .cloned()
                .ok_or_else(|| syn::Error::new_spanned(self, "expected ident")),
            _ => Err(syn::Error::new_spanned(self, "expected ident")),
        }
    }

    fn head(&self) -> syn::Result<Ident> {
        let Expr::Path(path) = self else {
            return Err(syn::Error::new_spanned(self, "expected type path"));
        };
        path.path
            .segments
            .last()
            .map(|seg| seg.ident.clone())
            .ok_or_else(|| syn::Error::new_spanned(self, "empty path"))
    }

    fn slot(&self) -> syn::Result<(String, Ident)> {
        let Expr::Assign(ExprAssign { left, right, .. }) = self else {
            return Err(syn::Error::new_spanned(
                self,
                "bond field form: name = atom",
            ));
        };
        Ok((left.ident()?.to_string(), right.ident()?.atom()?))
    }
}

pub(crate) trait Coin {
    fn atom(&self) -> syn::Result<Ident>;
    fn card(&self) -> syn::Result<Ident>;
}

impl Coin for Ident {
    fn atom(&self) -> syn::Result<Ident> {
        match self.to_string().as_str() {
            "string" => Ok(Ident::new("Text", self.span())),
            "url" => Ok(Ident::new("Link", self.span())),
            "int" => Ok(Ident::new("Int", self.span())),
            "bool" => Ok(Ident::new("Bool", self.span())),
            other => Err(syn::Error::new(
                self.span(),
                format!("unknown field atom: {other}"),
            )),
        }
    }

    fn card(&self) -> syn::Result<Ident> {
        match self.to_string().as_str() {
            "many2many" => Ok(Ident::new("Many2many", self.span())),
            "many2one" => Ok(Ident::new("Many2one", self.span())),
            "one2one" => Ok(Ident::new("One2one", self.span())),
            other => Err(syn::Error::new(
                self.span(),
                format!("unknown relation kind: {other}"),
            )),
        }
    }
}

pub(crate) trait Read {
    fn items(&self, hint: &str) -> syn::Result<Punctuated<Expr, Token![,]>>;
    fn only(&self, item: &Expr) -> syn::Result<Only>;
    fn scopes(&self, expr: &Expr) -> syn::Result<Vec<Scope>>;
    fn tally(&self, items: &Punctuated<Expr, Token![,]>) -> syn::Result<Tally>;
}

impl Read for Attribute {
    fn items(&self, hint: &str) -> syn::Result<Punctuated<Expr, Token![,]>> {
        let Meta::List(list) = &self.meta else {
            return Err(syn::Error::new_spanned(self, hint));
        };
        Punctuated::<Expr, Token![,]>::parse_terminated
            .parse2(list.tokens.clone())
            .map_err(|_| syn::Error::new_spanned(self, hint))
    }

    fn only(&self, item: &Expr) -> syn::Result<Only> {
        if item.ident().is_ok_and(|word| word == "unique") {
            return Ok(Only::All);
        }
        if let Expr::Assign(ExprAssign { left, right, .. }) = item
            && left.ident()? == "unique"
        {
            return Ok(Only::Per(self.scopes(right)?));
        }
        Err(syn::Error::new_spanned(
            self,
            "field extras: unique or unique = rel",
        ))
    }

    fn scopes(&self, expr: &Expr) -> syn::Result<Vec<Scope>> {
        let Expr::Tuple(tuple) = expr else {
            return Ok(vec![place(expr, "unique scope")?]);
        };
        let mut out = Vec::new();
        for item in &tuple.elems {
            out.push(place(item, "unique scope")?);
        }
        if out.is_empty() {
            return Err(syn::Error::new_spanned(self, "unique scope is empty"));
        }
        Ok(out)
    }

    fn tally(&self, items: &Punctuated<Expr, Token![,]>) -> syn::Result<Tally> {
        let hint = "serial needs scope = rel";
        let mut name = None;
        let mut scope = None;
        for item in items.iter().skip(1) {
            tally(item, &mut name, &mut scope, hint)?;
        }
        let scope = scope.ok_or_else(|| syn::Error::new_spanned(self, hint))?;
        Ok(Tally { name, scope })
    }
}

fn tally(
    item: &Expr,
    name: &mut Option<String>,
    scope: &mut Option<Scope>,
    hint: &str,
) -> syn::Result<()> {
    let Expr::Assign(ExprAssign { left, right, .. }) = item else {
        return Err(syn::Error::new_spanned(item, hint));
    };
    match left.ident()?.to_string().as_str() {
        "scope" => {
            if scope.is_some() {
                return Err(syn::Error::new_spanned(item, "duplicate serial scope"));
            }
            *scope = Some(place(right, "serial scope")?);
            Ok(())
        }
        "name" => put(name, right, item, "field name"),
        _ => Err(syn::Error::new_spanned(item, hint)),
    }
}

pub(crate) fn atom(attrs: &[Attribute]) -> syn::Result<Option<Made>> {
    let hint = "use #[field(string)] or #[field(string, default = \"ready\", values = [\"ready\", \"done\"])]";
    let mut found = None;
    for attr in attrs {
        if !attr.path().is_ident("field") {
            continue;
        }
        if found.is_some() {
            return Err(syn::Error::new_spanned(
                attr,
                "duplicate #[field(...)] attribute",
            ));
        }
        found = Some(Bud::parse(attr, hint)?);
    }
    Ok(found)
}

impl Bud {
    fn parse(attr: &Attribute, hint: &str) -> syn::Result<Made> {
        let items = attr.items(hint)?;
        if items.is_empty() {
            return Err(syn::Error::new_spanned(attr, "field takes one atom"));
        }
        let first = items[0].ident()?;
        if first == "serial" {
            return Ok(Made::Serial(attr.tally(&items)?));
        }
        let mut bud = Bud {
            name: None,
            kind: first.atom()?,
            only: Only::Free,
            need: true,
            guard: Guard::default(),
        };
        for item in items.iter().skip(1) {
            bud.option(attr, item)?;
        }
        Ok(Made::Atom(bud))
    }

    fn option(&mut self, attr: &Attribute, item: &Expr) -> syn::Result<()> {
        if let Ok(word) = item.ident() {
            return self.bare(attr, item, &word);
        }
        if let Expr::Assign(assign) = item {
            return self.keyed(attr, item, assign);
        }
        self.only = attr.only(item)?;
        Ok(())
    }

    fn bare(&mut self, attr: &Attribute, item: &Expr, word: &Ident) -> syn::Result<()> {
        match word.to_string().as_str() {
            "opt" => {
                if !self.need {
                    return Err(syn::Error::new_spanned(item, "duplicate field option: opt"));
                }
                self.need = false;
                Ok(())
            }
            "unique" => sole(&mut self.only, Only::All, item),
            "name" => Err(syn::Error::new_spanned(
                item,
                "field name needs name = \"...\"",
            )),
            _ => {
                self.only = attr.only(item)?;
                Ok(())
            }
        }
    }

    fn keyed(&mut self, attr: &Attribute, item: &Expr, assign: &ExprAssign) -> syn::Result<()> {
        match assign.left.ident()?.to_string().as_str() {
            "name" => put(&mut self.name, &assign.right, item, "field name"),
            "unique" => sole(&mut self.only, Only::Per(attr.scopes(&assign.right)?), item),
            _ => {
                let _ = crate::rule::read(attr, item, &mut self.only, &mut self.guard)?;
                Ok(())
            }
        }
    }
}

fn sole(only: &mut Only, next: Only, item: &Expr) -> syn::Result<()> {
    if !matches!(only, Only::Free) {
        return Err(syn::Error::new_spanned(
            item,
            "duplicate field option: unique",
        ));
    }
    *only = next;
    Ok(())
}
