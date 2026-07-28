use crate::*;
use syn::parse::Parser;
use syn::punctuated::Punctuated;
use syn::{Attribute, Expr, ExprAssign, Ident, Meta, Token, Type};

pub(crate) struct Link {
    pub card: Ident,
    pub target: String,
    pub slots: Vec<(String, Ident)>,
    pub need: bool,
    pub root: bool,
    pub crew: bool,
}

impl Link {
    fn shape(&self, attr: &Attribute) -> syn::Result<()> {
        if self.card != "Many2many" && !self.slots.is_empty() {
            return Err(syn::Error::new_spanned(attr, "only many2many takes fields"));
        }
        if self.card == "Many2many" && (!self.need || self.root) {
            return Err(syn::Error::new_spanned(
                attr,
                "opt and root are for single refs only",
            ));
        }
        if self.crew && self.card != "Many2many" {
            return Err(syn::Error::new_spanned(attr, "crew is many2many only"));
        }
        if self.root && !self.need {
            return Err(syn::Error::new_spanned(attr, "root is always required"));
        }
        Ok(())
    }
}

pub(crate) trait Spell {
    fn ident(&self) -> syn::Result<Ident>;
    fn tail(&self) -> syn::Result<String>;
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

    fn tail(&self) -> syn::Result<String> {
        let Expr::Path(path) = self else {
            return Ok(self.head()?.to_string());
        };
        let segs = &path.path.segments;
        if segs.len() == 2 {
            let root = segs[0].ident.to_string().to_ascii_lowercase();
            let name = segs[1].ident.to_string().to_ascii_lowercase();
            return Ok(format!("{root}:{name}"));
        }
        Ok(self.head()?.to_string())
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
    fn scopes(&self, expr: &Expr) -> syn::Result<Vec<String>>;
    fn tally(&self, items: &Punctuated<Expr, Token![,]>) -> syn::Result<String>;
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

    fn scopes(&self, expr: &Expr) -> syn::Result<Vec<String>> {
        let Expr::Tuple(tuple) = expr else {
            return Ok(vec![expr.ident()?.to_string()]);
        };
        let mut out = Vec::new();
        for item in &tuple.elems {
            out.push(item.ident()?.to_string());
        }
        if out.is_empty() {
            return Err(syn::Error::new_spanned(self, "unique scope is empty"));
        }
        Ok(out)
    }

    fn tally(&self, items: &Punctuated<Expr, Token![,]>) -> syn::Result<String> {
        let hint = "serial needs scope = rel";
        if items.len() != 2 {
            return Err(syn::Error::new_spanned(self, hint));
        }
        let Expr::Assign(ExprAssign { left, right, .. }) = &items[1] else {
            return Err(syn::Error::new_spanned(self, hint));
        };
        if left.ident()? != "scope" {
            return Err(syn::Error::new_spanned(self, hint));
        }
        Ok(right.ident()?.to_string())
    }
}

pub(crate) fn atom(attrs: &[Attribute]) -> syn::Result<Option<Made>> {
    let hint = "use #[field(string)] or #[field(string, default = \"ready\", values = (\"ready\", \"done\"))]";
    for attr in attrs {
        if !attr.path().is_ident("field") {
            continue;
        }
        let items = attr.items(hint)?;
        if items.is_empty() {
            return Err(syn::Error::new_spanned(attr, "field takes one atom"));
        }
        let first = items[0].ident()?;
        if first == "serial" {
            return Ok(Some(Made::Serial(attr.tally(&items)?)));
        }
        let kind = first.atom()?;
        let mut only = Only::Free;
        let mut need = true;
        let mut guard = Guard::default();
        for item in items.iter().skip(1) {
            if item.ident().is_ok_and(|word| word == "opt") {
                need = false;
                continue;
            }
            if crate::rule::read(attr, item, &mut only, &mut guard)? {
                continue;
            }
            only = attr.only(item)?;
        }
        return Ok(Some(Made::Atom(kind, only, need, guard)));
    }
    Ok(None)
}

pub(crate) fn link(attrs: &[Attribute], ty: &Type) -> syn::Result<Option<Link>> {
    let hint = "use #[relation(Type, many2many, field = atom)] or #[relation(Type, many2one, opt)]";
    for attr in attrs {
        if !attr.path().is_ident("relation") {
            continue;
        }
        let items = attr.items(hint)?;
        if items.len() < 2 {
            return Err(syn::Error::new_spanned(
                attr,
                "use #[relation(Type, many2many)]",
            ));
        }
        let mut link = Link {
            card: items[1].head()?.card()?,
            target: items[0].tail()?,
            slots: Vec::new(),
            need: true,
            root: false,
            crew: false,
        };
        for item in items.iter().skip(2) {
            match item.ident().ok().map(|word| word.to_string()).as_deref() {
                Some("opt") => link.need = false,
                Some("root") => link.root = true,
                Some("crew") => link.crew = true,
                _ => link.slots.push(item.slot()?),
            }
        }
        link.shape(attr)?;
        let _ = ty;
        return Ok(Some(link));
    }
    Ok(None)
}
