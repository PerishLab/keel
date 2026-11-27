use super::failed;
use syn::{ItemStruct, parse_quote};

#[test]
fn invalid() {
    let base: ItemStruct = parse_quote! {
        struct Item {
            #[field(string)]
            code: string,
        }
    };
    let cases = [
        (parse_quote!(name = ""), "must not be empty"),
        (parse_quote!(name = "   "), "must not be empty"),
        (parse_quote!(name = "Bad Name"), "whitespace"),
        (parse_quote!(name = 123), "nonempty string literal"),
        (parse_quote!(name), "name = \"...\""),
        (parse_quote!(other = "X"), "unknown resource option"),
        (parse_quote!(mystery), "unknown resource mode"),
        (
            parse_quote!(name = "A", name = "B"),
            "duplicate resource name",
        ),
        (parse_quote!(veil, veil), "duplicate resource option"),
        (parse_quote!(frozen, frozen), "duplicate resource option"),
    ];
    for (attr, want) in cases {
        let note = failed(attr, base.clone());
        assert!(note.contains(want), "{note}");
    }
}

#[test]
fn duplicate() {
    let cases: [(ItemStruct, &str); 6] = [
        (
            parse_quote! {
                struct Item {
                    #[field(string, name = "a", name = "b")]
                    code: string,
                }
            },
            "duplicate field name",
        ),
        (
            parse_quote! {
                struct Item {
                    #[field(string, name = "code")]
                    alpha: string,
                    #[field(string, name = "Code")]
                    beta: string,
                }
            },
            "duplicate field name",
        ),
        (
            parse_quote! {
                struct Item {
                    #[relation(Org, many2one, name = "a", name = "b")]
                    org: Org,
                }
            },
            "duplicate relation name",
        ),
        (
            parse_quote! {
                struct Item {
                    #[field(serial, scope = org, scope = other)]
                    number: int,
                }
            },
            "duplicate serial scope",
        ),
        (
            parse_quote! {
                struct Item {
                    #[field(serial, scope = org, name = "a", name = "b")]
                    number: int,
                }
            },
            "duplicate field name",
        ),
        (
            parse_quote! {
                struct Item {
                    #[field(string, unique, unique = org)]
                    code: string,
                }
            },
            "duplicate field option",
        ),
    ];
    for (item, want) in cases {
        let note = failed(parse_quote!(), item);
        assert!(note.contains(want), "{note}");
    }
}

#[test]
fn malformed() {
    let cases: [(ItemStruct, &str); 9] = [
        (
            parse_quote! {
                struct Item {
                    #[field(string, name = "")]
                    code: string,
                }
            },
            "must not be empty",
        ),
        (
            parse_quote! {
                struct Item {
                    #[field(string, name = 123)]
                    code: string,
                }
            },
            "nonempty string literal",
        ),
        (
            parse_quote! {
                struct Item {
                    #[field(string, name)]
                    code: string,
                }
            },
            "name = \"...\"",
        ),
        (
            parse_quote! {
                struct Item {
                    #[relation(Org, many2one, name = "")]
                    org: Org,
                }
            },
            "must not be empty",
        ),
        (
            parse_quote! {
                struct Item {
                    #[relation(Org, many2one, name = string)]
                    org: Org,
                }
            },
            "nonempty string literal",
        ),
        (
            parse_quote! {
                struct Item {
                    #[relation("", many2one)]
                    org: Org,
                }
            },
            "must not be empty",
        ),
        (
            parse_quote! {
                struct Item {
                    #[field(serial, name = "seq_no")]
                    number: int,
                }
            },
            "serial needs scope = rel",
        ),
        (
            parse_quote! {
                struct Item {
                    #[field(string, unique = "")]
                    code: string,
                }
            },
            "must not be empty",
        ),
        (
            parse_quote! {
                struct Item {
                    #[field(string)]
                    #[relation(Org, many2one)]
                    both: Org,
                }
            },
            "both",
        ),
    ];
    for (item, want) in cases {
        let note = failed(parse_quote!(), item);
        assert!(note.contains(want), "{note}");
    }
}
