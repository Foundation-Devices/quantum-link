use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::parse_macro_input;

/// Implements `ql_codec::Encode` and `ql_codec::Decode`, generating `<Name>Ref` as the borrowed
/// form unless the type has no fields
///
/// See the `ql_codec` crate docs for the encoding and the `#[codec(...)]` attributes.
#[proc_macro_derive(Codec, attributes(codec))]
pub fn derive_codec_entry(item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    derive_codec(parse_macro_input!(item as syn::DeriveInput))
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}

fn derive_codec(item: syn::DeriveInput) -> Result<TokenStream, syn::Error> {
    if !item.generics.params.is_empty() {
        return Err(syn::Error::new_spanned(
            &item.generics,
            "Generic types are not supported",
        ));
    }
    let options = Options::parse(&item.attrs)?;
    let expansion = match &item.data {
        syn::Data::Struct(_) if options.discriminants.is_some() => {
            return Err(syn::Error::new_spanned(
                &item.ident,
                "Only enums have discriminants",
            ))
        }
        syn::Data::Struct(data) => derive_struct(&item, Fields::new(&data.fields, options.frozen)?),
        syn::Data::Enum(data) => derive_enum(&item, data, &options)?,
        syn::Data::Union(_) => {
            return Err(syn::Error::new_spanned(
                &item.ident,
                "Unions are not supported",
            ))
        }
    };
    Ok(expand(&item, &options.derives, expansion))
}

#[derive(Default)]
struct Options {
    frozen: bool,
    derives: Vec<syn::Path>,
    discriminants: Option<syn::Ident>,
}

impl Options {
    fn parse(attrs: &[syn::Attribute]) -> Result<Self, syn::Error> {
        let mut options = Self::default();
        for attr in attrs.iter().filter(|attr| attr.path().is_ident("codec")) {
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("frozen") {
                    options.frozen = true;
                    Ok(())
                } else if meta.path.is_ident("derive") {
                    meta.parse_nested_meta(|derive| {
                        options.derives.push(derive.path);
                        Ok(())
                    })
                } else if meta.path.is_ident("discriminants") {
                    options.discriminants = Some(meta.value()?.parse()?);
                    Ok(())
                } else {
                    Err(meta.error("Expected `frozen`, `derive(...)` or `discriminants = Name`"))
                }
            })?;
        }
        Ok(options)
    }
}

/// The parts that differ between structs and enums
struct Expansion {
    /// Declaration of the borrowed form, if the type has fields to borrow
    ///
    /// A struct or enum without fields is its own borrowed form instead, because a
    /// `<Name>Ref<'a>` with no field using `'a` doesn't compile.
    ref_declaration: Option<TokenStream>,
    decode_ref: TokenStream,
    from_ref: TokenStream,
    decode: TokenStream,
    missing: TokenStream,
    encoded_len: TokenStream,
    encode: TokenStream,
    /// The discriminants enum and its accessor, if requested
    discriminants: TokenStream,
}

/// The form a decode builds
#[derive(Clone, Copy)]
enum Form {
    Owned,
    Borrowed,
}

impl Form {
    /// The form's type, as named inside the `Decode` impl
    fn path(self) -> TokenStream {
        match self {
            Self::Owned => quote!(Self),
            Self::Borrowed => quote!(Self::Ref),
        }
    }
}

fn expand(item: &syn::DeriveInput, derives: &[syn::Path], expansion: Expansion) -> TokenStream {
    let Expansion {
        ref_declaration,
        decode_ref,
        from_ref,
        decode,
        missing,
        encoded_len,
        encode,
        discriminants,
    } = expansion;
    let ident = &item.ident;
    let ref_ident = ref_ident(ident);

    let mut encoded = vec![quote!(#ident)];
    let (ref_items, ref_type) = match ref_declaration {
        Some(ref_declaration) => {
            let doc =
                format!("[`{ident}`] with byte buffers and strings borrowed from the encoding");
            let derives = (!derives.is_empty()).then(|| quote!(#[derive(#(#derives),*)]));
            encoded.push(quote!(#ref_ident<'_>));
            let ref_items = quote!(
                #[doc = #doc]
                #derives
                #ref_declaration

                impl #ref_ident<'_> {
                    /// Copies the borrowed data into the owned type
                    #[allow(dead_code)]
                    pub fn into_owned(self) -> #ident {
                        <#ident as ::ql_codec::Decode>::from_ref(self)
                    }
                }

                #[automatically_derived]
                impl ::core::convert::From<#ref_ident<'_>> for #ident {
                    fn from(value: #ref_ident<'_>) -> Self {
                        <Self as ::ql_codec::Decode>::from_ref(value)
                    }
                }
            );
            (ref_items, quote!(#ref_ident<'a>))
        }
        None => (quote!(), quote!(Self)),
    };
    // Both forms bind their fields through `Self`, so they share the encoding.
    let encode_impls = encoded.iter().map(|ty| {
        quote!(
            #[automatically_derived]
            impl ::ql_codec::Encode for #ty {
                fn encoded_len(&self) -> usize {
                    #encoded_len
                }

                fn encode<W: ::ql_codec::BufMut + ?Sized>(&self, out: &mut W) {
                    #encode
                }
            }

            #[automatically_derived]
            impl ::ql_codec::Element for #ty {}
        )
    });

    quote!(
        #ref_items
        #discriminants

        #[automatically_derived]
        impl ::ql_codec::Decode for #ident {
            type Ref<'a> = #ref_type;

            fn decode_ref<'a>(
                reader: &mut ::ql_codec::Reader<'a>,
            ) -> ::core::result::Result<Self::Ref<'a>, ::ql_codec::Error> {
                #decode_ref
            }

            fn from_ref(value: Self::Ref<'_>) -> Self {
                #from_ref
            }

            fn decode(
                reader: &mut ::ql_codec::Reader<'_>,
            ) -> ::core::result::Result<Self, ::ql_codec::Error> {
                #decode
            }

            fn missing<'a>() -> ::core::option::Option<Self::Ref<'a>> {
                #missing
            }
        }

        #(#encode_impls)*
    )
}

fn derive_struct(item: &syn::DeriveInput, fields: Fields) -> Expansion {
    let vis = &item.vis;
    let ref_ident = ref_ident(&item.ident);

    let pattern = fields.pattern();
    let decode_prelude = decode_prelude(fields.frozen);
    let decode = |form: Form| {
        let value = fields.decode(&form.path(), form);
        quote!(#decode_prelude ::core::result::Result::Ok(#value))
    };
    let encoded_len = fields.encoded_len();
    let encode = fields.encode();
    let shape = fields.ref_shape();
    let semicolon = matches!(fields.shape, syn::Fields::Unnamed(_)).then(|| quote!(;));
    let owned = fields.owned(&quote!(Self));

    Expansion {
        ref_declaration: (!fields.fields.is_empty())
            .then(|| quote!(#vis struct #ref_ident<'a> #shape #semicolon)),
        decode_ref: decode(Form::Borrowed),
        from_ref: quote!(let Self::Ref #pattern = value; #owned),
        decode: decode(Form::Owned),
        missing: fields.missing(&quote!(Self::Ref)),
        encoded_len: quote!(let Self #pattern = self; #encoded_len),
        encode: quote!(let Self #pattern = self; #encode),
        discriminants: quote!(),
    }
}

struct Variant<'a> {
    ident: &'a syn::Ident,
    attrs: &'a [syn::Attribute],
    enumerator: u32,
    fields: Fields<'a>,
}

fn derive_enum(
    item: &syn::DeriveInput,
    data: &syn::DataEnum,
    options: &Options,
) -> Result<Expansion, syn::Error> {
    let frozen = options.frozen;
    let vis = &item.vis;
    let ref_ident = ref_ident(&item.ident);

    let mut variants = Vec::new();
    let mut unknown = None;
    let mut next_enumerator = Some(0u32);
    for variant in &data.variants {
        let enumerator = match &variant.discriminant {
            Some((_, discriminant)) => parse_enumerator(discriminant)?,
            None => next_enumerator
                .ok_or_else(|| syn::Error::new_spanned(variant, "Enumerator overflows u32"))?,
        };
        next_enumerator = enumerator.checked_add(1);

        if has_flag(&variant.attrs, "unknown")? {
            if frozen {
                return Err(syn::Error::new_spanned(
                    variant,
                    "A frozen enum has no unknown variant",
                ));
            }
            if !matches!(variant.fields, syn::Fields::Unit) {
                return Err(syn::Error::new_spanned(
                    variant,
                    "The unknown variant must be a unit variant",
                ));
            }
            if unknown.is_some() {
                return Err(syn::Error::new_spanned(
                    variant,
                    "Only one variant can be unknown",
                ));
            }
            unknown = Some(&variant.ident);
        }

        variants.push(Variant {
            ident: &variant.ident,
            attrs: &variant.attrs,
            enumerator,
            fields: Fields::new(&variant.fields, frozen)?,
        });
    }

    let borrows = variants
        .iter()
        .any(|variant| !variant.fields.fields.is_empty());

    let encoded_len_arms = variants.iter().map(|variant| {
        let Variant {
            ident,
            enumerator,
            fields,
            ..
        } = variant;
        let cfg = cfg_attrs(variant.attrs);
        let pattern = fields.pattern();
        let encoded_len = fields.encoded_len();
        quote!(#(#cfg)* Self::#ident #pattern => ::ql_codec::varint::encoded_len(#enumerator) + #encoded_len,)
    });
    let encode_arms = variants.iter().map(|variant| {
        let Variant {
            ident,
            enumerator,
            fields,
            ..
        } = variant;
        let cfg = cfg_attrs(variant.attrs);
        let pattern = fields.pattern();
        let encode = fields.encode();
        quote!(#(#cfg)* Self::#ident #pattern => {
            ::ql_codec::varint::encode(#enumerator, out);
            #encode
        })
    });
    // `match self {}` is rejected for a reference to an empty enum.
    let scrutinee = if variants.is_empty() {
        quote!(*self)
    } else {
        quote!(self)
    };

    let decode_prelude = decode_prelude(frozen);
    let decode = |form: Form| {
        let path = form.path();
        let arms = variants.iter().map(|variant| {
            let Variant {
                ident,
                enumerator,
                fields,
                ..
            } = variant;
            let cfg = cfg_attrs(variant.attrs);
            let value = fields.decode(&quote!(#path::#ident), form);
            quote!(#(#cfg)* #enumerator => #value,)
        });
        let unknown_arm = match unknown {
            Some(unknown) => quote!(_ => #path::#unknown {},),
            None => quote!(
                _ => return ::core::result::Result::Err(::ql_codec::Error::InvalidDiscriminant),
            ),
        };
        quote!(
            let enumerator = reader.decode_varint::<u32>()?;
            #decode_prelude
            ::core::result::Result::Ok(match enumerator {
                #(#arms)*
                #unknown_arm
            })
        )
    };

    let ref_variants = variants.iter().map(|variant| {
        let attrs = ref_attrs(variant.attrs);
        let ident = variant.ident;
        let shape = variant.fields.ref_shape();
        quote!(#(#attrs)* #ident #shape,)
    });
    let from_ref_arms = variants.iter().map(|variant| {
        let Variant { ident, fields, .. } = variant;
        let cfg = cfg_attrs(variant.attrs);
        let pattern = fields.pattern();
        let owned = fields.owned(&quote!(Self::#ident));
        quote!(#(#cfg)* Self::Ref::#ident #pattern => #owned,)
    });

    let discriminants = options.discriminants.as_ref().map(|name| {
        let ident = &item.ident;
        let doc = format!("[`{ident}`]'s variants without their fields, numbered by enumerator");
        let discriminants = variants.iter().map(|variant| {
            let cfg = cfg_attrs(variant.attrs);
            let ident = variant.ident;
            let enumerator = proc_macro2::Literal::u32_unsuffixed(variant.enumerator);
            quote!(#(#cfg)* #ident = #enumerator,)
        });
        let arms = variants.iter().map(|variant| {
            let cfg = cfg_attrs(variant.attrs);
            let ident = variant.ident;
            quote!(#(#cfg)* Self::#ident { .. } => #name::#ident,)
        });
        quote!(
            #[doc = #doc]
            #[derive(Debug, Clone, Copy, PartialEq, Eq)]
            #vis enum #name { #(#discriminants)* }

            impl #ident {
                /// The variant without its fields
                #[allow(dead_code)]
                #vis fn discriminant(&self) -> #name {
                    match #scrutinee { #(#arms)* }
                }
            }
        )
    });

    Ok(Expansion {
        ref_declaration: borrows.then(|| quote!(#vis enum #ref_ident<'a> { #(#ref_variants)* })),
        decode_ref: decode(Form::Borrowed),
        from_ref: quote!(match value { #(#from_ref_arms)* }),
        decode: decode(Form::Owned),
        missing: match unknown {
            Some(unknown) => quote!(::core::option::Option::Some(Self::Ref::#unknown {})),
            None => quote!(::core::option::Option::None),
        },
        encoded_len: quote!(match #scrutinee { #(#encoded_len_arms)* }),
        encode: quote!(match #scrutinee { #(#encode_arms)* }),
        discriminants: discriminants.unwrap_or_default(),
    })
}

struct Fields<'a> {
    shape: &'a syn::Fields,
    fields: Vec<Field<'a>>,
    frozen: bool,
}

struct Field<'a> {
    attrs: &'a [syn::Attribute],
    vis: &'a syn::Visibility,
    member: syn::Member,
    binding: syn::Ident,
    ty: &'a syn::Type,
}

impl<'a> Fields<'a> {
    fn new(shape: &'a syn::Fields, frozen: bool) -> Result<Self, syn::Error> {
        let fields = shape
            .iter()
            .enumerate()
            .map(|(index, field)| {
                if let Some(attr) = field
                    .attrs
                    .iter()
                    .find(|attr| attr.path().is_ident("codec"))
                {
                    return Err(syn::Error::new_spanned(
                        attr,
                        "Fields take no codec attributes",
                    ));
                }
                Ok(Field {
                    attrs: &field.attrs,
                    vis: &field.vis,
                    member: field
                        .ident
                        .clone()
                        .map_or_else(|| syn::Member::Unnamed(index.into()), syn::Member::Named),
                    binding: format_ident!("field_{index}"),
                    ty: &field.ty,
                })
            })
            .collect::<Result<_, _>>()?;
        Ok(Self {
            shape,
            fields,
            frozen,
        })
    }

    /// `{ a: field_0, 1: field_1 }`, which also matches tuple and unit shapes
    fn pattern(&self) -> TokenStream {
        let fields = self.fields.iter().map(|field| {
            let Field {
                member, binding, ..
            } = field;
            let cfg = cfg_attrs(field.attrs);
            quote!(#(#cfg)* #member: #binding,)
        });
        quote!({ #(#fields)* })
    }

    /// Length of the fields bound by [`Self::pattern`]
    fn encoded_len(&self) -> TokenStream {
        let fields_len = self.fields_len();
        if self.frozen {
            fields_len
        } else {
            quote!(::ql_codec::encoded_len_prefixed(#fields_len))
        }
    }

    /// Encodes the fields bound by [`Self::pattern`] into `out`
    fn encode(&self) -> TokenStream {
        let fields = self.fields.iter().map(|field| {
            let binding = &field.binding;
            let cfg = cfg_attrs(field.attrs);
            quote!(#(#cfg)* ::ql_codec::Encode::encode(#binding, out);)
        });
        let len = (!self.frozen).then(|| {
            let fields_len = self.fields_len();
            quote!(::ql_codec::encode_len_prefix(#fields_len, out);)
        });
        quote!(#len #(#fields)*)
    }

    fn fields_len(&self) -> TokenStream {
        let fields = self.fields.iter().map(|field| {
            let binding = &field.binding;
            let cfg = cfg_attrs(field.attrs);
            quote!(#(#cfg)* { len += ::ql_codec::Encode::encoded_len(#binding); })
        });
        quote!({
            #[allow(unused_mut)]
            let mut len = 0usize;
            #(#fields)*
            len
        })
    }

    fn decode(&self, path: &TokenStream, form: Form) -> TokenStream {
        self.construct(path, |ty| match (self.frozen, form) {
            (true, Form::Owned) => quote!(<#ty as ::ql_codec::Decode>::decode(reader)?),
            (true, Form::Borrowed) => quote!(<#ty as ::ql_codec::Decode>::decode_ref(reader)?),
            (false, Form::Owned) => quote!(::ql_codec::decode_field::<#ty>(&mut fields)?),
            (false, Form::Borrowed) => quote!(::ql_codec::decode_field_ref::<#ty>(&mut fields)?),
        })
    }

    fn missing(&self, path: &TokenStream) -> TokenStream {
        let value = self.construct(path, |ty| quote!(<#ty as ::ql_codec::Decode>::missing()?));
        quote!(::core::option::Option::Some(#value))
    }

    /// Builds the owned value from the borrowed fields bound by [`Self::pattern`]
    fn owned(&self, path: &TokenStream) -> TokenStream {
        let fields = self.fields.iter().map(|field| {
            let Field {
                member,
                binding,
                ty,
                ..
            } = field;
            let cfg = cfg_attrs(field.attrs);
            quote!(#(#cfg)* #member: <#ty as ::ql_codec::Decode>::from_ref(#binding),)
        });
        quote!(#path { #(#fields)* })
    }

    fn construct(
        &self,
        path: &TokenStream,
        value: impl Fn(&syn::Type) -> TokenStream,
    ) -> TokenStream {
        let fields = self.fields.iter().map(|field| {
            let member = &field.member;
            let cfg = cfg_attrs(field.attrs);
            let value = value(field.ty);
            quote!(#(#cfg)* #member: #value,)
        });
        quote!(#path { #(#fields)* })
    }

    /// The fields of the borrowed form, in the declared shape
    fn ref_shape(&self) -> TokenStream {
        let fields = self.fields.iter().map(|field| {
            let Field { vis, ty, .. } = field;
            let attrs = ref_attrs(field.attrs);
            let name = match &field.member {
                syn::Member::Named(ident) => quote!(#ident:),
                syn::Member::Unnamed(_) => quote!(),
            };
            quote!(#(#attrs)* #vis #name <#ty as ::ql_codec::Decode>::Ref<'a>,)
        });
        match self.shape {
            syn::Fields::Named(_) => quote!({ #(#fields)* }),
            syn::Fields::Unnamed(_) => quote!(( #(#fields)* )),
            syn::Fields::Unit => quote!(),
        }
    }
}

/// Takes the length-prefixed fields out of `reader` as `fields`, for [`Fields::decode`]
fn decode_prelude(frozen: bool) -> TokenStream {
    if frozen {
        return quote!();
    }
    quote!(
        // Unused without fields, but the body still has to be skipped.
        #[allow(unused_mut, unused_variables)]
        let mut fields = ::ql_codec::Reader::new(reader.take_len_prefixed()?);
    )
}

/// Whether `attrs` hold `#[codec(name)]`, the only codec attribute allowed there
fn has_flag(attrs: &[syn::Attribute], name: &str) -> Result<bool, syn::Error> {
    let mut found = false;
    for attr in attrs.iter().filter(|attr| attr.path().is_ident("codec")) {
        attr.parse_nested_meta(|meta| {
            if !meta.path.is_ident(name) {
                return Err(meta.error(format!("Expected `{name}`")));
            }
            found = true;
            Ok(())
        })?;
    }
    Ok(found)
}

fn parse_enumerator(discriminant: &syn::Expr) -> Result<u32, syn::Error> {
    let syn::Expr::Lit(syn::ExprLit {
        lit: syn::Lit::Int(int),
        ..
    }) = discriminant
    else {
        return Err(syn::Error::new_spanned(
            discriminant,
            "Discriminant must be an integer literal",
        ));
    };
    int.base10_parse()
}

fn ref_ident(ident: &syn::Ident) -> syn::Ident {
    format_ident!("{ident}Ref")
}

fn cfg_attrs(attrs: &[syn::Attribute]) -> Vec<&syn::Attribute> {
    attrs
        .iter()
        .filter(|attr| attr.path().is_ident("cfg"))
        .collect()
}

/// Attributes carried over to the borrowed form
fn ref_attrs(attrs: &[syn::Attribute]) -> Vec<&syn::Attribute> {
    attrs
        .iter()
        .filter(|attr| attr.path().is_ident("cfg") || attr.path().is_ident("doc"))
        .collect()
}
