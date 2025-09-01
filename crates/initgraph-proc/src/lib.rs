use proc_macro::TokenStream;
use syn::{
    Attribute, Expr, Ident, Path, Token, Type, Visibility,
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
};

struct Task {
    attrs: Vec<TaskAttribute>,
    vis: Visibility,
    ident: Ident,
    value: Expr,
}

#[derive(Debug)]
enum TaskAttribute {
    Passthrough(Attribute),
    Depends(Vec<Path>),
    Entails(Vec<Path>),
}

fn path_matches(path: &Path, parts: &[&str]) -> bool {
    path.segments.len() == parts.len()
        && path
            .segments
            .iter()
            .map(|seg| seg.ident.to_string())
            .zip(parts.iter().copied())
            .all(|(a, b)| a == b)
}

fn parse_paths(attr: &Attribute) -> syn::Result<Vec<Path>> {
    let paths: Vec<_> = attr
        .parse_args_with(Punctuated::<Path, Token![,]>::parse_separated_nonempty)?
        .iter()
        .cloned()
        .collect();

    Ok(paths)
}

impl Parse for Task {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let attrs = Attribute::parse_outer(&input)?
            .into_iter()
            .map(|attr| {
                if path_matches(attr.path(), &["initgraph", "depends"]) {
                    parse_paths(&attr).map(TaskAttribute::Depends)
                } else if path_matches(attr.path(), &["initgraph", "entails"]) {
                    parse_paths(&attr).map(TaskAttribute::Entails)
                } else {
                    Ok(TaskAttribute::Passthrough(attr))
                }
            })
            .collect::<syn::Result<Vec<_>>>()?;

        let vis = input.parse()?;
        let _static: Token![static] = input.parse()?;
        let ident = input.parse()?;
        let _colon: Token![:] = input.parse()?;
        let _ty: Type = input.parse()?;
        let _equal: Token![=] = input.parse()?;
        let value: Expr = input.parse()?;
        let _semi: Token![;] = input.parse()?;

        Ok(Self {
            attrs,
            vis,
            ident,
            value,
        })
    }
}

#[proc_macro_attribute]
pub fn task(attr: TokenStream, item: TokenStream) -> TokenStream {
    let Task {
        attrs,
        vis,
        ident,
        value,
    } = syn::parse_macro_input!(item);

    let task_name: syn::LitStr = syn::parse_macro_input!(attr);

    let passthrough_attrs = attrs.iter().filter_map(|attr| {
        if let TaskAttribute::Passthrough(attr) = attr {
            Some(attr)
        } else {
            None
        }
    });

    let edges = attrs
        .iter()
        .filter(|attr| matches!(attr, TaskAttribute::Depends(_) | TaskAttribute::Entails(_)))
        .flat_map(|attr| match attr {
            TaskAttribute::Passthrough(_) => todo!(),
            TaskAttribute::Depends(paths) => paths
                .iter()
                .map(|path| quote::quote! { ::initgraph::Edge::new(&#path, &#ident) })
                .collect::<Vec<_>>(),
            TaskAttribute::Entails(paths) => paths
                .iter()
                .map(|path| quote::quote! { ::initgraph::Edge::new(&#ident, &#path) })
                .collect::<Vec<_>>(),
        })
        .map(|edge| {
            quote::quote! {
                const _: () = {
                    #[used]
                    #[doc(hidden)]
                    static __EDGE: ::initgraph::Edge = #edge;

                    #[used]
                    #[doc(hidden)]
                    #[unsafe(link_section = ".init.ctors")]
                    static __EDGE_CTOR: fn() = || __EDGE.register();
                };
            }
        });

    quote::quote! {
        #(#passthrough_attrs)*
        #[used]
        #[doc(hidden)]
        #[unsafe(link_section = ".init.nodes")]
        #vis static #ident: ::initgraph::Node =
            ::initgraph::Node::new(#task_name, ::initgraph::NodeAction::Callback(#value));

        #(#edges)*
    }
    .into()
}
