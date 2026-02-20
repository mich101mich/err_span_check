use proc_macro2::TokenStream;
use quote::{ToTokens, quote, quote_spanned};
use syn::Result;

/// A simple procedural macro
#[proc_macro]
pub fn my_basic_macro(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(input as basic::Input);
    match input.produce_output() {
        Ok(output) => output.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

mod basic {
    use super::*;

    pub struct Input {
        a: String,
        b: syn::Type,
        c: syn::Expr,
    }

    impl syn::parse::Parse for Input {
        fn parse(input: syn::parse::ParseStream) -> Result<Self> {
            let a = input.parse::<syn::LitStr>()?.value();
            input.parse::<syn::Token![,]>()?;
            let b = input.parse()?;
            input.parse::<syn::Token![,]>()?;
            let c = input.parse()?;
            Ok(Self { a, b, c })
        }
    }

    impl Input {
        pub fn produce_output(&self) -> Result<TokenStream> {
            assert!(!self.a.contains('!'), "the literal should not contain '!'");
            let Input { a, b, c } = self;
            Ok(quote! {
                let x = #c; // make sure we only evaluate c once
                println!("{}: {}", #a, x);
                let _: #b = x;
            })
        }
    }
}

/// A seemingly simple procedural macro, but it goes out of its way to produce good error messages
#[proc_macro]
pub fn my_advanced_macro(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(input as advanced::Input);
    match input.produce_output() {
        Ok(output) => output.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

mod advanced {
    use super::*;

    pub struct Input {
        a: proc_macro2::Literal,
        b: syn::Type,
        c: syn::Expr,
    }

    fn err_at_end<T>(token: impl syn::spanned::Spanned, msg: &str) -> syn::Result<T> {
        let span = token.span().unwrap().end().into(); // replace with span().end() once syn adds support for it
        Err(syn::Error::new(span, msg))
    }

    impl syn::parse::Parse for Input {
        fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
            if input.is_empty() {
                return Err(syn::Error::new(
                    proc_macro2::Span::call_site(), // can't do any better than this
                    "my_advanced_macro! requires 3 parameters: a string literal, a type, and an expression",
                ));
            }

            let a = input.parse::<syn::LitStr>()?.token();

            if input.is_empty() {
                // When there are no more tokens, syn has no choice but to point at the entire macro invocation, which
                // is not very helpful. Instead, we manually point at the last successfully parsed token.
                let msg = "expected 3 parameters, found 1. Missing: a type and an expression";
                return err_at_end(a, msg);
            }
            let comma = input.parse::<syn::Token![,]>()?;

            if input.is_empty() {
                let msg = "expected 3 parameters, found 1. Missing: a type and an expression";
                return err_at_end(comma, msg);
            }

            let b = input.parse()?;

            if input.is_empty() {
                let msg = "expected 3 parameters, found 2. Missing: an expression";
                return err_at_end(b, msg);
            }

            let comma = input.parse::<syn::Token![,]>()?;

            if input.is_empty() {
                let msg = "expected 3 parameters, found 2. Missing: an expression";
                return err_at_end(comma, msg);
            }

            let c = input.parse()?;

            // If the proc macro does not consume all input, there will be a really confusing error about "unexpected token"
            let rest = input.parse::<proc_macro2::TokenStream>().unwrap();
            if !rest.is_empty() {
                let msg = "expected exactly 3 parameters, but found extra inputs";
                return Err(syn::Error::new_spanned(rest, msg));
            }

            Ok(Self { a, b, c })
        }
    }

    impl Input {
        pub fn produce_output(&self) -> syn::Result<proc_macro2::TokenStream> {
            let Input { a, b, c } = self;

            if let Some(pos) = a.to_string().find('!') {
                if let Some(span) = a.subspan(pos..=pos) {
                    return Err(syn::Error::new(span, "the literal should not contain '!'"));
                } else {
                    // we are on stable and subspan hasn't been stabilized yet
                    return Err(syn::Error::new_spanned(
                        a,
                        "the literal should not contain '!'",
                    ));
                }
            }

            let access = {
                // workaround: we want to store the input expression c in a temporary variable to ensure it's only
                // evaluated once, but we need any errors related to c to point at the original expression, not the
                // temporary variable. To achieve this, we need the produced expression to have the same span as c.
                // On stable, this requires multiple tokens, so we store the value in a struct, since the ".0" field
                // access has multiple tokens.
                // Note that this is exactly what syn::Error::new_spanned does internally, except we need to apply it
                // to an arbitrary expression since we don't directly produce an error here.
                let mut iter = c.to_token_stream().into_iter();
                let start = iter
                    .next()
                    .map(|t| t.span())
                    .unwrap_or_else(proc_macro2::Span::call_site);
                let end = iter.last().map(|t| t.span()).unwrap_or(start);
                let start_tokens = quote_spanned! {start=> x};
                let end_tokens = quote_spanned! {end=> .0};
                quote! { #start_tokens #end_tokens }
            };

            Ok(quote! {{
                struct SpanHackWrapper<T>(T);
                let x = SpanHackWrapper(#c); // make sure we only evaluate c once
                println!("{}: {}", #a, #access);
                let _: #b = #access;
            }})
        }
    }
}
