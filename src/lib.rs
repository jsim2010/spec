//! Provides a `spec` attribute that defines a specification.
extern crate proc_macro;

use ::rustfmt::{config::Config, Input};
use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{quote, ToTokens};
use syn::{
    braced, parse::ParseStream, parse_macro_input, parse_macro_input::ParseMacroInput, Block,
    Ident, Item, Lit, Meta, Result, Stmt, Token, Type,
};

/// Defines a specification statement.
enum SpecStatement {
    /// A statement using "shall".
    ///
    /// "`ident` shall {}"
    Shall(String),
    /// A conditional statement.
    ///
    /// "If `ident`, {}"
    Cond(String),
}

impl SpecStatement {
    /// Returns the full statement.
    fn stmt(&self, ident: &str) -> String {
        match self {
            SpecStatement::Shall(s) => format!("`{}` shall {}", ident, s),
            SpecStatement::Cond(s) => format!("If `{}` {}", ident, s),
        }
    }
}

impl Default for SpecStatement {
    fn default() -> Self {
        SpecStatement::Shall(String::default())
    }
}

/// Defines a specification.
#[derive(Default)]
struct Spec {
    /// The name of the specification
    name: String,
    /// The statement of the specification.
    stmt: SpecStatement,
    /// The `Stmt`s that certify the specification has been met.
    cert: Vec<Stmt>,
}

impl ParseMacroInput for Spec {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let ident = input.parse::<Ident>()?;
        let mut spec = Self::default();

        if ident.to_string().as_str() == "name" && input.parse::<Token![=]>().is_ok() {
            spec.name = match input.parse::<Lit>()? {
                Lit::Str(lit_str) => Ok(lit_str.value()),
                _ => Err(input.error("Error parsing spec name")),
            }?;

            let _ = input.parse::<Token![,]>()?;
            let ident = input.parse::<Ident>()?;

            if ident.to_string().as_str() == "shall" && input.parse::<Token![=]>().is_ok() {
                spec.stmt = match input.parse::<Lit>()? {
                    Lit::Str(lit_str) => Ok(SpecStatement::Shall(lit_str.value())),
                    _ => Err(input.error("Error parsing spec shall statement value")),
                }?;
            } else if ident.to_string().as_str() == "cond" && input.parse::<Token![=]>().is_ok() {
                spec.stmt = match input.parse::<Lit>()? {
                    Lit::Str(lit_str) => Ok(SpecStatement::Cond(lit_str.value())),
                    _ => Err(input.error("Error parsing spec cond statement value")),
                }?;
            } else {
                return Err(input.error("Expected spec shall or cond"));
            }

            if !input.is_empty() {
                let _ = input.parse::<Token![,]>()?;
                let ident = input.parse::<Ident>()?;

                if ident.to_string().as_str() == "cert" {
                    let content;
                    braced!(content in input);
                    spec.cert = content.call(Block::parse_within)?;
                } else {
                    return Err(input.error("Expected spec cert"));
                }
            }
        } else {
            return Err(input.error("Expected spec name"));
        }

        Ok(spec)
    }
}

/// Adds a specification to the item.
#[proc_macro_attribute]
#[inline]
pub fn spec(args: TokenStream, item: TokenStream) -> TokenStream {
    let spec = parse_macro_input!(args as Spec);
    let item = parse_macro_input!(item as Item);
    let mut item_attrs = Vec::new();
    let mut after_attrs = quote! {};
    let mut item_ident = Ident::new("_", Span::call_site());

    match item {
        Item::Enum(item_enum) => {
            let vis = item_enum.vis;
            let generics = item_enum.generics;
            let variants = item_enum.variants;

            item_ident = item_enum.ident;
            item_attrs = item_enum.attrs;
            after_attrs = quote! {
                #vis enum #item_ident #generics {
                    #variants
                }
            };
        }
        Item::Impl(item_impl) => {
            let defaultness = item_impl.defaultness;
            let unsafety = item_impl.unsafety;
            let generics = item_impl.generics;
            let mut trait_path = TokenStream2::new();
            let mut trait_for = TokenStream2::new();

            if let Some(tr) = item_impl.trait_ {
                trait_path = tr.1.into_token_stream();
                trait_for = tr.2.into_token_stream();
            }

            let items = item_impl.items;
            let self_ty = *item_impl.self_ty.clone();

            if let Type::Path(path) = self_ty.clone() {
                if let Some(segment) = path.path.segments.last() {
                    item_ident = segment.value().ident.clone();
                }
            }

            item_attrs = item_impl.attrs;
            after_attrs = quote! {
                #defaultness #unsafety impl #generics #trait_path #trait_for #self_ty {
                    #(#items)*
                }
            };
        }
        Item::Fn(item_fn) => {
            let vis = item_fn.vis;
            let constness = item_fn.constness;
            let unsafety = item_fn.unsafety;
            let asyncness = item_fn.asyncness;
            let abi = item_fn.abi;
            let generics = item_fn.decl.generics;
            let inputs = item_fn.decl.inputs;
            let variadic = item_fn.decl.variadic;
            let output = item_fn.decl.output;
            let block = item_fn.block;

            item_ident = item_fn.ident;
            item_attrs = item_fn.attrs;
            after_attrs = quote! {
                #vis #constness #unsafety #asyncness #abi fn #item_ident #generics(#inputs #variadic) #output {
                    #block
                }
            };
        }
        _ => {}
    }

    let mut title_doc = "# Specifications";

    for attr in &item_attrs {
        if let Ok(Meta::NameValue(name_value)) = attr.parse_meta() {
            if name_value.ident.to_string().as_str() == "doc" {
                if let Lit::Str(lit_str) = name_value.lit {
                    if lit_str.value() == title_doc {
                        title_doc = "";
                        break;
                    }
                }
            }
        }
    }

    let name_doc = format!("## SPEC-{}-{}", item_ident, spec.name);
    let stmt_doc = format!("> {}.\n", spec.stmt.stmt(&item_ident.to_string()));
    let mut cert_doc = String::default();

    if !spec.cert.is_empty() {
        let mut example = String::from("fn main() {\n");
        let fmt_config = Config::default();

        for stmt in spec.cert {
            example.push_str(&stmt.into_token_stream().to_string());
        }

        example.push_str("}");

        if let Ok(output) =
            rustfmt::format_input::<std::io::Stdout>(Input::Text(example), &fmt_config, None)
        {
            cert_doc.push_str("```\n");
            for record in output.1 {
                let fmt_example = record.1.to_string();
                let length = fmt_example.lines().count().saturating_sub(2);

                for line in fmt_example.lines().skip(1).take(length) {
                    if let Some(trimmed_line) = line.get(4..) {
                        cert_doc.push_str(trimmed_line);
                        cert_doc.push('\n');
                    }
                }
            }

            cert_doc.push_str("\n```");
        }
    };

    let output = quote! {
        #(#item_attrs)*
        #[doc = #title_doc]
        #[doc = #name_doc]
        #[doc = #stmt_doc]
        #[doc = #cert_doc]
        #after_attrs
    };

    TokenStream::from(output)
}
