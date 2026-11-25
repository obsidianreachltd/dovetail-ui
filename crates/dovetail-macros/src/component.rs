//! `#[component]`: a function becomes a component type, a properties
//! struct, and a builder that checks required properties at compile time.

use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote, quote_spanned};
use syn::spanned::Spanned;
use syn::{Expr, FnArg, Ident, ItemFn, Pat, ReturnType, Type};

/// How a property gets its value when it is left out.
enum Fallback {
  Required,
  TypeDefault,
  Value(Expr),
}

struct Prop {
  name: Ident,
  ty: Type,
  default: Fallback,
  /// `#[prop(into)]`: the setter takes anything that converts into the type.
  into: bool,
  /// Takes a closure, for `Callback<T>` properties: the event type.
  callback_of: Option<Type>,
}

pub fn expand(
  attr: TokenStream,
  item: TokenStream,
) -> syn::Result<TokenStream> {
  if !attr.is_empty() {
    return Err(syn::Error::new(
      attr.span(),
      "`#[component]` takes no arguments",
    ));
  }
  let mut function: ItemFn = syn::parse2(item)?;
  let sig = &function.sig;
  if !sig.generics.params.is_empty() || sig.generics.where_clause.is_some() {
    return Err(syn::Error::new(
      sig.generics.span(),
      "components cannot have generic parameters yet",
    ));
  }
  if sig.asyncness.is_some() {
    return Err(syn::Error::new(
      sig.asyncness.span(),
      "components cannot be `async`",
    ));
  }
  if let ReturnType::Default = sig.output {
    return Err(syn::Error::new(
      sig.ident.span(),
      "a component must return `View`",
    ));
  }

  let name = sig.ident.clone();
  let vis = function.vis.clone();
  let mut props = Vec::new();
  for input in function.sig.inputs.iter_mut() {
    let FnArg::Typed(arg) = input else {
      return Err(syn::Error::new(
        input.span(),
        "a component cannot take `self`",
      ));
    };
    let Pat::Ident(pat) = &*arg.pat else {
      return Err(syn::Error::new(
        arg.pat.span(),
        "each property must be a plain name, such as `label: String`",
      ));
    };
    let mut default = Fallback::Required;
    let mut into = false;
    let mut kept = Vec::new();
    for a in arg.attrs.drain(..) {
      if !a.path().is_ident("prop") {
        kept.push(a);
        continue;
      }
      a.parse_nested_meta(|meta| {
        if meta.path.is_ident("default") {
          default = if meta.input.peek(syn::Token![=]) {
            Fallback::Value(meta.value()?.parse()?)
          } else {
            Fallback::TypeDefault
          };
          Ok(())
        } else if meta.path.is_ident("into") {
          into = true;
          Ok(())
        } else {
          Err(meta.error(
            "unknown property option; use `default`, `default = value` or `into`",
          ))
        }
      })?;
    }
    arg.attrs = kept;
    let ty = (*arg.ty).clone();
    if pat.ident == "children"
      && matches!(default, Fallback::Required)
      && last_segment_is(&ty, "Children")
    {
      default = Fallback::TypeDefault;
    }
    props.push(Prop {
      name: pat.ident.clone(),
      callback_of: callback_type(&ty),
      ty,
      default,
      into,
    });
  }

  let props_name = format_ident!("{}Props", name);
  let builder_name = format_ident!("__{}PropsBuilder", name);
  let required: Vec<&Prop> = props
    .iter()
    .filter(|p| matches!(p.default, Fallback::Required))
    .collect();
  let markers: Vec<Ident> = required
    .iter()
    .map(|p| format_ident!("__{}", p.name))
    .collect();

  let field_names: Vec<&Ident> = props.iter().map(|p| &p.name).collect();
  let field_types: Vec<&Type> = props.iter().map(|p| &p.ty).collect();

  // The struct of properties.
  let doc = format!("The properties of [`{name}`].");
  let props_struct = quote! {
    #[doc = #doc]
    #[derive(Clone, PartialEq)]
    #vis struct #props_name {
      #( #[allow(missing_docs)] #vis #field_names: #field_types, )*
    }
  };

  // The builder, with one marker type parameter per required property.
  let unset = quote!(::dovetail::__private::Unset);
  let set = quote!(::dovetail::__private::Set);
  let all_unset = markers.iter().map(|_| unset.clone());
  let builder = quote! {
    #[doc(hidden)]
    #[allow(non_camel_case_types)]
    #vis struct #builder_name<#(#markers,)*> {
      #( #field_names: ::core::option::Option<#field_types>, )*
      __markers: ::core::marker::PhantomData<(#(#markers,)*)>,
    }

    impl #props_name {
      #[doc(hidden)]
      #vis fn __builder() -> #builder_name<#(#all_unset,)*> {
        #builder_name {
          #( #field_names: ::core::option::Option::None, )*
          __markers: ::core::marker::PhantomData,
        }
      }
    }
  };

  // One setter per property.
  let mut setters = TokenStream::new();
  for prop in &props {
    let field = &prop.name;
    let ty = &prop.ty;
    let (param, value) = match &prop.callback_of {
      Some(event) => (
        quote!(value: impl ::core::ops::Fn(#event) + 'static),
        quote!(::dovetail::Callback::new(value)),
      ),
      None if prop.into => (
        quote!(value: impl ::core::convert::Into<#ty>),
        quote!(::core::convert::Into::into(value)),
      ),
      // The exact type, so literals such as `3` take the property's type.
      None => (quote!(value: #ty), quote!(value)),
    };
    let others: Vec<TokenStream> = props
      .iter()
      .filter(|p| p.name != prop.name)
      .map(|p| {
        let n = &p.name;
        quote!(#n: self.#n)
      })
      .collect();
    let setter_doc = format!("Sets the `{field}` property.");
    match markers
      .iter()
      .position(|m| *m == format_ident!("__{}", field))
    {
      Some(index) => {
        let generics: Vec<&Ident> = markers
          .iter()
          .enumerate()
          .filter(|(i, _)| *i != index)
          .map(|(_, m)| m)
          .collect();
        let before: Vec<TokenStream> = markers
          .iter()
          .enumerate()
          .map(|(i, m)| {
            if i == index {
              unset.clone()
            } else {
              quote!(#m)
            }
          })
          .collect();
        let after: Vec<TokenStream> = markers
          .iter()
          .enumerate()
          .map(|(i, m)| if i == index { set.clone() } else { quote!(#m) })
          .collect();
        setters.extend(quote! {
          #[allow(non_camel_case_types)]
          impl<#(#generics,)*> #builder_name<#(#before,)*> {
            #[doc = #setter_doc]
            #vis fn #field(self, #param) -> #builder_name<#(#after,)*> {
              #builder_name {
                #field: ::core::option::Option::Some(#value),
                #(#others,)*
                __markers: ::core::marker::PhantomData,
              }
            }
          }
        });
      },
      None => setters.extend(quote! {
        #[allow(non_camel_case_types)]
        impl<#(#markers,)*> #builder_name<#(#markers,)*> {
          #[doc = #setter_doc]
          #vis fn #field(mut self, #param) -> Self {
            self.#field = ::core::option::Option::Some(#value);
            self
          }
        }
      }),
    }
  }

  // `__build` exists only when every required property is set. Each
  // missing one names itself in the error, through its own trait.
  let mut needs = TokenStream::new();
  let mut bounds = Vec::new();
  for (prop, marker) in required.iter().zip(&markers) {
    let trait_name = format_ident!("__{}Needs_{}", name, prop.name);
    let message = format!("`{name}` needs the property `{}`", prop.name);
    let label = format!("`{}` is missing here", prop.name);
    // `on_unimplemented` treats braces as placeholders, so they are doubled.
    let note = format!(
      "give it inside the braces, for example `{name} {{{{ {}: ... }}}}`",
      prop.name
    );
    needs.extend(quote! {
      #[doc(hidden)]
      #[allow(non_camel_case_types)]
      #[diagnostic::on_unimplemented(message = #message, label = #label, note = #note)]
      #vis trait #trait_name {}
      impl #trait_name for #set {}
    });
    bounds.push(quote!(#marker: #trait_name));
  }
  let values = props.iter().map(|p| {
    let n = &p.name;
    match &p.default {
      Fallback::Required => quote!(#n: builder.#n.expect("required properties are checked when compiling")),
      Fallback::TypeDefault => quote!(#n: builder.#n.unwrap_or_default()),
      Fallback::Value(expr) if p.into => {
        quote_spanned!(expr.span()=> #n: builder.#n.unwrap_or_else(|| ::core::convert::Into::into(#expr)))
      },
      Fallback::Value(expr) => quote_spanned!(expr.span()=> #n: builder.#n.unwrap_or_else(|| #expr)),
    }
  });
  // An associated function rather than a method: an unmet bound on a
  // method call is reported as "method exists but its trait bounds were not
  // satisfied", which ignores `on_unimplemented`. On a function argument it
  // is a plain unmet bound, and the message names the missing property.
  let build = quote! {
    #needs
    #[allow(non_camel_case_types)]
    impl #props_name {
      #[doc(hidden)]
      #vis fn __build<#(#markers,)*>(builder: #builder_name<#(#markers,)*>) -> Self
      where
        #(#bounds,)*
      {
        Self { #(#values,)* }
      }
    }
  };

  // The component type and its build, which runs the original function.
  let attrs = std::mem::take(&mut function.attrs);
  function.sig.ident = Ident::new("__render", Span::mixed_site());
  function.vis = syn::Visibility::Inherited;
  let component = quote! {
    #(#attrs)*
    #vis struct #name;

    impl ::dovetail::Component for #name {
      type Props = #props_name;

      fn build(
        __props: &#props_name,
        _: &mut ::dovetail::BuildContext<'_>,
      ) -> ::dovetail::View {
        #[allow(non_snake_case, clippy::too_many_arguments)]
        #function
        let #props_name { #(#field_names,)* } = ::core::clone::Clone::clone(__props);
        __render(#(#field_names,)*)
      }
    }
  };

  Ok(quote! { #component #props_struct #builder #setters #build })
}

fn last_segment_is(
  ty: &Type,
  name: &str,
) -> bool {
  matches!(ty, Type::Path(p) if p.path.segments.last().is_some_and(|s| s.ident == name))
}

/// `T` for a property of type `Callback<T>`.
fn callback_type(ty: &Type) -> Option<Type> {
  let Type::Path(p) = ty else {
    return None;
  };
  let last = p.path.segments.last()?;
  if last.ident != "Callback" {
    return None;
  }
  let syn::PathArguments::AngleBracketed(args) = &last.arguments else {
    return None;
  };
  match args.args.first()? {
    syn::GenericArgument::Type(t) => Some(t.clone()),
    _ => None,
  }
}
