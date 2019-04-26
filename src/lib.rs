extern crate proc_macro;

use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{quote, ToTokens};
use syn::{braced, parse_macro_input, Ident, Item, Lit, parse_macro_input::ParseMacroInput, Stmt, Type, Block, Token, Result, parse::ParseStream};

#[derive(Default)]
struct Spec {
    name: String,
    shall: String,
    cert: Vec<Stmt>,
}

impl ParseMacroInput for Spec {
    fn parse(input: ParseStream) -> Result<Self> {
        let ident = input.parse::<Ident>()?;
        let mut spec = Spec::default();

        if ident.to_string().as_str() == "name" && input.parse::<Token![=]>().is_ok() {
            spec.name = match input.parse::<Lit>()? {
                Lit::Str(lit_str) => {
                    Ok(lit_str.value())
                }
                _ => Err(input.error("Error parsing spec name")),
            }?;

            input.parse::<Token![,]>()?;
            let ident = input.parse::<Ident>()?;

            if ident.to_string().as_str() == "shall" && input.parse::<Token![=]>().is_ok() {
                spec.shall = match input.parse::<Lit>()? {
                    Lit::Str(lit_str) => {
                        Ok(lit_str.value())
                    }
                    _ => Err(input.error("Error parsing spec shall statement value"))
                }?;

                if !input.is_empty() {
                    input.parse::<Token![,]>()?;
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
                return Err(input.error("Expected spec shall"));
            }
        } else {
            return Err(input.error("Expected spec name"));
        }

        Ok(spec)
    }
}

#[proc_macro_attribute]
pub fn spec(args: TokenStream, item: TokenStream) -> TokenStream {
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
        _ => {}
    }

    let spec = parse_macro_input!(args as Spec);
    let name_doc = format!("## SPEC-{}-{}", item_ident, spec.name);
    let stmt_doc = format!("> `{}` shall {}.\n", item_ident, spec.shall);
    let cert_doc = if spec.cert.is_empty() {
        String::default()
    } else {
        let mut d = String::from("```\n");

        for stmt in spec.cert {
            d.push_str(&stmt.into_token_stream().to_string());
            d.push('\n');
        }

        d.push_str("\n```");
        d
    };

    let output = quote! {
        #(#item_attrs)*
        #[doc = "# Specifications"]
        #[doc = #name_doc]
        #[doc = #stmt_doc]
        #[doc = #cert_doc]
        #after_attrs
    };

    TokenStream::from(output)
}
