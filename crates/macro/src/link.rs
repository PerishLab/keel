use crate::head::{Aim, aim, flag, put};
use crate::parse::{Coin, Read, Spell};
use syn::{Attribute, Expr, Ident, Type};

pub(crate) struct Link {
    pub name: Option<String>,
    pub card: Ident,
    pub target: Aim,
    pub slots: Vec<(String, Ident)>,
    pub need: bool,
    pub root: bool,
    pub crew: bool,
    pub closure: bool,
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
        if self.closure && self.card != "Many2many" {
            return Err(syn::Error::new_spanned(attr, "closure is many2many only"));
        }
        if self.root && !self.need {
            return Err(syn::Error::new_spanned(attr, "root is always required"));
        }
        Ok(())
    }

    fn edge(&mut self, item: &Expr) -> syn::Result<()> {
        if let Ok(word) = item.ident() {
            return self.mode(item, &word);
        }
        if let Expr::Assign(assign) = item
            && assign.left.ident()? == "name"
        {
            return self.alias(item, &assign.right);
        }
        self.slots.push(item.slot()?);
        Ok(())
    }

    fn mode(&mut self, item: &Expr, word: &Ident) -> syn::Result<()> {
        match word.to_string().as_str() {
            "opt" => {
                if !self.need {
                    return Err(syn::Error::new_spanned(
                        item,
                        "duplicate relation option: opt",
                    ));
                }
                self.need = false;
                Ok(())
            }
            "root" => flag(&mut self.root, item, "relation", "root"),
            "crew" => flag(&mut self.crew, item, "relation", "crew"),
            "closure" => flag(&mut self.closure, item, "relation", "closure"),
            "name" => Err(syn::Error::new_spanned(
                item,
                "relation name needs name = \"...\"",
            )),
            _ => {
                self.slots.push(item.slot()?);
                Ok(())
            }
        }
    }

    fn alias(&mut self, item: &Expr, right: &Expr) -> syn::Result<()> {
        if self.card == "Many2many"
            && let Ok(atom) = right.ident().and_then(|word| word.atom())
        {
            self.slots.push(("name".into(), atom));
            return Ok(());
        }
        put(&mut self.name, right, item, "relation name")
    }
}

pub(crate) fn link(attrs: &[Attribute], ty: &Type) -> syn::Result<Option<Link>> {
    let hint = "use #[relation(Type, many2many, field = atom)] or #[relation(Type, many2one, opt)]";
    let mut found = None;
    for attr in attrs {
        if !attr.path().is_ident("relation") {
            continue;
        }
        if found.is_some() {
            return Err(syn::Error::new_spanned(
                attr,
                "duplicate #[relation(...)] attribute",
            ));
        }
        found = Some(bond(attr, ty, hint)?);
    }
    Ok(found)
}

fn bond(attr: &Attribute, ty: &Type, hint: &str) -> syn::Result<Link> {
    let items = attr.items(hint)?;
    if items.len() < 2 {
        return Err(syn::Error::new_spanned(
            attr,
            "use #[relation(Type, many2many)]",
        ));
    }
    let mut link = Link {
        name: None,
        card: items[1].head()?.card()?,
        target: aim(&items[0], ty)?,
        slots: Vec::new(),
        need: true,
        root: false,
        crew: false,
        closure: false,
    };
    for item in items.iter().skip(2) {
        link.edge(item)?;
    }
    link.shape(attr)?;
    Ok(link)
}
