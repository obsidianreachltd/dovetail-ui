//! `ui!`: parses a tree of nodes and expands it to M02 calls.
//!
//! ```text
//! nodes     = node*
//! node      = component | "text" | { expr } | if | for
//! component = Path { (name: expr ,?)* nodes }
//! if        = if expr { nodes } (else if ... | else { nodes })?
//! for       = for pat in expr (, key = expr)? { node }
//! ```

use std::collections::HashSet;

use proc_macro2::TokenStream;
use quote::{quote, quote_spanned};
use syn::parse::{Parse, ParseStream};
use syn::spanned::Spanned;
use syn::{Expr, Ident, LitStr, Pat, Path, Token, braced};

struct Nodes(Vec<Node>);

enum Node {
  Component {
    path: Path,
    props: Vec<(Ident, Expr)>,
    children: Vec<Node>,
  },
  Text(LitStr),
  Expr(Box<Expr>),
  If {
    cond: Box<Expr>,
    then: Nodes,
    otherwise: Option<Box<Node>>,
  },
  Else(Nodes),
  For(Box<ForNode>),
}

struct ForNode {
  pat: Pat,
  iter: Expr,
  key: Option<Expr>,
  body: Node,
  span: proc_macro2::Span,
}

mod kw {
  syn::custom_keyword!(key);
}

impl Parse for Nodes {
  fn parse(input: ParseStream) -> syn::Result<Self> {
    let mut nodes = Vec::new();
    while !input.is_empty() {
      nodes.push(input.parse()?);
    }
    Ok(Nodes(nodes))
  }
}

impl Parse for Node {
  fn parse(input: ParseStream) -> syn::Result<Self> {
    if input.peek(LitStr) {
      return Ok(Node::Text(input.parse()?));
    }
    if input.peek(syn::token::Brace) {
      let content;
      braced!(content in input);
      return Ok(Node::Expr(Box::new(content.parse()?)));
    }
    if input.peek(Token![if]) {
      return parse_if(input);
    }
    if input.peek(Token![for]) {
      let span = input.parse::<Token![for]>()?.span;
      let pat = Pat::parse_single(input)?;
      input.parse::<Token![in]>()?;
      let iter = Expr::parse_without_eager_brace(input)?;
      let key = if input.peek(Token![,]) {
        input.parse::<Token![,]>()?;
        input.parse::<kw::key>()?;
        input.parse::<Token![=]>()?;
        Some(Expr::parse_without_eager_brace(input)?)
      } else {
        None
      };
      let content;
      braced!(content in input);
      let Nodes(mut body) = content.parse()?;
      if body.len() != 1 {
        return Err(syn::Error::new(
          span,
          "the body of a `for` must be exactly one node",
        ));
      }
      return Ok(Node::For(Box::new(ForNode {
        pat,
        iter,
        key,
        body: body.remove(0),
        span,
      })));
    }
    if input.peek(Ident)
      || input.peek(Token![::])
      || input.peek(Token![crate])
      || input.peek(Token![self])
      || input.peek(Token![super])
    {
      let path: Path = input.call(Path::parse_mod_style)?;
      let content;
      braced!(content in input);
      let mut props = Vec::new();
      let mut seen = HashSet::new();
      while content.peek(Ident)
        && content.peek2(Token![:])
        && !content.peek2(Token![::])
      {
        let name: Ident = content.parse()?;
        content.parse::<Token![:]>()?;
        let value: Expr = content.parse()?;
        if name == "children" {
          return Err(syn::Error::new(
            name.span(),
            "write children as nodes inside the braces, not as `children:`",
          ));
        }
        if !seen.insert(name.to_string()) {
          return Err(syn::Error::new(
            name.span(),
            format!("the property `{name}` is given more than once"),
          ));
        }
        props.push((name, value));
        if content.peek(Token![,]) {
          content.parse::<Token![,]>()?;
        }
      }
      let Nodes(children) = content.parse()?;
      return Ok(Node::Component { path, props, children });
    }
    Err(input.error("expected a component, a \"text\" literal, `{ expression }`, `if` or `for`"))
  }
}

fn parse_if(input: ParseStream) -> syn::Result<Node> {
  input.parse::<Token![if]>()?;
  let cond = Box::new(Expr::parse_without_eager_brace(input)?);
  let content;
  braced!(content in input);
  let then: Nodes = content.parse()?;
  let otherwise = if input.peek(Token![else]) {
    input.parse::<Token![else]>()?;
    if input.peek(Token![if]) {
      Some(Box::new(parse_if(input)?))
    } else {
      let content;
      braced!(content in input);
      Some(Box::new(Node::Else(content.parse()?)))
    }
  } else {
    None
  };
  Ok(Node::If { cond, then, otherwise })
}

pub fn expand(input: TokenStream) -> syn::Result<TokenStream> {
  let nodes: Nodes = syn::parse2(input)?;
  Ok(nodes_to_view(&nodes.0))
}

fn nodes_to_view(nodes: &[Node]) -> TokenStream {
  match nodes {
    [] => quote!(::dovetail::View::Empty),
    [one] => node_to_view(one),
    many => {
      let views = many.iter().map(node_to_view);
      quote!(::dovetail::View::fragment([#(#views,)*]))
    },
  }
}

fn node_to_view(node: &Node) -> TokenStream {
  match node {
    Node::Text(text) => {
      quote_spanned!(text.span()=> ::dovetail::View::text(#text))
    },
    Node::Expr(expr) => {
      quote_spanned!(expr.span()=> ::dovetail::__private::into_view(#expr))
    },
    Node::Else(nodes) => nodes_to_view(&nodes.0),
    Node::If { cond, then, otherwise } => {
      let then = nodes_to_view(&then.0);
      let otherwise = match otherwise {
        Some(node) => node_to_view(node),
        None => quote!(::dovetail::View::Empty),
      };
      quote!(if #cond { #then } else { #otherwise })
    },
    Node::For(for_node) => {
      let ForNode { pat, iter, key, body, span } = &**for_node;
      let body = node_to_view(body);
      let body = match key {
        Some(key) => {
          quote_spanned!(key.span()=> ::dovetail::View::keyed(#body, #key))
        },
        None => body,
      };
      quote_spanned!(*span=> ::dovetail::View::fragment(
        ::core::iter::IntoIterator::into_iter(#iter).map(|#pat| #body)
      ))
    },
    Node::Component { path, props, children } => {
      let span = path.span();
      let setters = props
        .iter()
        .map(|(name, value)| quote_spanned!(name.span()=> .#name(#value)));
      let children = if children.is_empty() {
        TokenStream::new()
      } else {
        let views = children.iter().map(node_to_view);
        quote_spanned!(span=> .children(::std::vec![#(#views,)*]))
      };
      let build = quote_spanned!(span=> __Props::__build);
      quote_spanned!(span=> {
        type __Props = <#path as ::dovetail::Component>::Props;
        ::dovetail::View::component::<#path>(#build(__Props::__builder() #(#setters)* #children))
      })
      // The braces make the alias local to this node.
    },
  }
}
