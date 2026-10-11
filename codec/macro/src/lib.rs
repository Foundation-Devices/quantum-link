use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{parse_macro_input, parse_quote};

/// implements the codec traits and generates `<Name>Ref<B>` unless the type has no fields
///
/// See the `ql_codec` crate docs for the encoding and the `#[codec(...)]` attributes.
#[proc_macro_derive(Codec, attributes(codec))]
pub fn derive_codec_entry(item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    derive_codec(parse_macro_input!(item as syn::DeriveInput))
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}

fn derive_codec(item: syn::DeriveInput) -> Result<TokenStream, syn::Error> {
    // borrowed fields are parameterized by their input container
    if let Some(lifetime) = item.generics.lifetimes().next() {
        return Err(syn::Error::new_spanned(
            lifetime,
            "Lifetime parameters are not supported",
        ));
    }
    let options = Options::parse(&item.attrs)?;
    let mut input = format_ident!("B");
    while item.generics.params.iter().any(|param| match param {
        syn::GenericParam::Type(param) => param.ident == input,
        syn::GenericParam::Const(param) => param.ident == input,
        syn::GenericParam::Lifetime(_) => false,
    }) {
        input = format_ident!("{input}_");
    }
    let ref_generics = ref_generics(&item, &input);
    let expansion = match &item.data {
        syn::Data::Struct(_) if options.discriminants.is_some() => {
            return Err(syn::Error::new_spanned(
                &item.ident,
                "Only enums have discriminants",
            ))
        }
        syn::Data::Struct(data) => derive_struct(
            &item,
            &ref_generics,
            Fields::new(&data.fields, options.frozen, &input)?,
        ),
        syn::Data::Enum(data) => derive_enum(&item, &ref_generics, data, &options, &input)?,
        syn::Data::Union(_) => {
            return Err(syn::Error::new_spanned(
                &item.ident,
                "Unions are not supported",
            ))
        }
    };
    Ok(expand(
        &item,
        &ref_generics,
        &options.derives,
        &input,
        expansion,
    ))
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
    /// declaration of the borrowed form when the type has fields
    ref_declaration: Option<TokenStream>,
    decode_ref: TokenStream,
    from_ref: TokenStream,
    decode: TokenStream,
    missing: TokenStream,
    missing_owned: TokenStream,
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

fn expand(
    item: &syn::DeriveInput,
    ref_generics: &syn::Generics,
    derives: &[syn::Path],
    input: &syn::Ident,
    expansion: Expansion,
) -> TokenStream {
    let Expansion {
        ref_declaration,
        decode_ref,
        from_ref,
        decode,
        missing,
        missing_owned,
        encoded_len,
        encode,
        discriminants,
    } = expansion;
    let ident = &item.ident;
    let ref_ident = ref_ident(ident);
    let (_, ty_generics, _) = item.generics.split_for_impl();
    let (ref_impl_generics, ref_ty_generics, ref_bounds) = ref_generics.split_for_impl();

    let encode_impl = |generics: syn::Generics, ty: TokenStream| {
        let (impl_generics, _, where_clause) = generics.split_for_impl();
        quote!(
            #[automatically_derived]
            impl #impl_generics ::ql_codec::Encode for #ty #where_clause {
                #[inline]
                fn encoded_len(&self) -> usize {
                    #encoded_len
                }

                fn encode<W: ::ql_codec::BufMut + ?Sized>(&self, out: &mut W) {
                    #encode
                }
            }
        )
    };
    let owned_encode = encode_impl(
        with_field_bounds(
            &item.generics,
            item,
            |ty| parse_quote!(#ty: ::ql_codec::Encode),
        ),
        quote!(#ident #ty_generics),
    );
    let (ref_items, ref_type) = match ref_declaration {
        Some(ref_declaration) => {
            let doc = format!("[`{ident}`] with fields borrowed from its input container");
            let derives = (!derives.is_empty()).then(|| quote!(#[derive(#(#derives),*)]));
            let ref_encode = encode_impl(
                with_field_bounds(
                    ref_generics,
                    item,
                    |ty| parse_quote!(<#ty as ::ql_codec::Decode<#input>>::Ref: ::ql_codec::Encode),
                ),
                quote!(#ref_ident #ref_ty_generics),
            );
            let ref_items = quote!(
                #[doc = #doc]
                #derives
                #ref_declaration

                impl #ref_impl_generics #ref_ident #ref_ty_generics #ref_bounds {
                    /// copies borrowed data into the owned type
                    #[allow(dead_code)]
                    #[inline]
                    pub fn into_owned(self) -> #ident #ty_generics {
                        self.into()
                    }
                }

                #[automatically_derived]
                impl #ref_impl_generics ::core::convert::From<#ref_ident #ref_ty_generics>
                    for #ident #ty_generics #ref_bounds
                {
                    #[inline]
                    fn from(value: #ref_ident #ref_ty_generics) -> Self {
                        <Self as ::ql_codec::Decode<#input>>::from_ref(value)
                    }
                }

                #ref_encode
            );
            (ref_items, quote!(#ref_ident #ref_ty_generics))
        }
        None => (quote!(), quote!(Self)),
    };

    quote!(
        #ref_items
        #discriminants

        #[automatically_derived]
        impl #ref_impl_generics ::ql_codec::Decode<#input> for #ident #ty_generics #ref_bounds {
            type Ref = #ref_type;

            #[inline]
            fn decode(reader: &mut ::ql_codec::Reader<#input>)
                -> ::core::result::Result<Self, ::ql_codec::Error>
            {
                #decode
            }

            #[inline]
            fn decode_ref(reader: &mut ::ql_codec::Reader<#input>)
                -> ::core::result::Result<Self::Ref, ::ql_codec::Error>
            {
                #decode_ref
            }

            #[inline]
            fn from_ref(value: Self::Ref) -> Self {
                #from_ref
            }

            fn missing() -> ::core::option::Option<Self> {
                #missing_owned
            }

            #[inline]
            fn missing_ref() -> ::core::option::Option<Self::Ref> {
                #missing
            }
        }

        #owned_encode
    )
}

fn derive_struct(
    item: &syn::DeriveInput,
    ref_generics: &syn::Generics,
    fields: Fields,
) -> Expansion {
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
    let where_clause = &ref_generics.where_clause;
    let ref_declaration = match fields.shape {
        syn::Fields::Unnamed(_) => {
            quote!(#vis struct #ref_ident #ref_generics #shape #where_clause;)
        }
        _ => quote!(#vis struct #ref_ident #ref_generics #where_clause #shape),
    };
    let owned = fields.owned(&quote!(Self));

    Expansion {
        ref_declaration: (!fields.shape.is_empty()).then_some(ref_declaration),
        decode_ref: decode(Form::Borrowed),
        from_ref: quote!(let Self::Ref #pattern = value; #owned),
        decode: decode(Form::Owned),
        missing: fields.missing(Form::Borrowed),
        missing_owned: fields.missing(Form::Owned),
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
    ref_generics: &syn::Generics,
    data: &syn::DataEnum,
    options: &Options,
    input: &syn::Ident,
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
            fields: Fields::new(&variant.fields, frozen, input)?,
        });
    }

    let borrows = variants
        .iter()
        .any(|variant| !variant.fields.shape.is_empty());

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
        let (impl_generics, ty_generics, where_clause) = item.generics.split_for_impl();
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

            impl #impl_generics #ident #ty_generics #where_clause {
                /// The variant without its fields
                #[allow(dead_code)]
                #[inline]
                #vis fn discriminant(&self) -> #name {
                    match #scrutinee { #(#arms)* }
                }
            }
        )
    });

    let where_clause = &ref_generics.where_clause;
    Ok(Expansion {
        ref_declaration: borrows.then(
            || quote!(#vis enum #ref_ident #ref_generics #where_clause { #(#ref_variants)* }),
        ),
        decode_ref: decode(Form::Borrowed),
        from_ref: quote!(match value { #(#from_ref_arms)* }),
        decode: decode(Form::Owned),
        missing: match unknown {
            Some(unknown) => quote!(::core::option::Option::Some(Self::Ref::#unknown {})),
            None => quote!(::core::option::Option::None),
        },
        missing_owned: match unknown {
            Some(unknown) => quote!(::core::option::Option::Some(Self::#unknown {})),
            None => quote!(::core::option::Option::None),
        },
        encoded_len: quote!(match #scrutinee { #(#encoded_len_arms)* }),
        encode: quote!(match #scrutinee { #(#encode_arms)* }),
        discriminants: discriminants.unwrap_or_default(),
    })
}

struct Fields<'a> {
    shape: &'a syn::Fields,
    frozen: bool,
    input: &'a syn::Ident,
}

impl<'a> Fields<'a> {
    fn new(
        shape: &'a syn::Fields,
        frozen: bool,
        input: &'a syn::Ident,
    ) -> Result<Self, syn::Error> {
        if let Some(attr) = shape
            .iter()
            .flat_map(|field| &field.attrs)
            .find(|attr| attr.path().is_ident("codec"))
        {
            return Err(syn::Error::new_spanned(
                attr,
                "Fields take no codec attributes",
            ));
        }
        Ok(Self {
            shape,
            frozen,
            input,
        })
    }

    /// `{ a: field_0, 1: field_1 }` also matches tuple and unit shapes
    fn pattern(&self) -> TokenStream {
        let fields = self.shape.iter().enumerate().map(|(index, field)| {
            let member = field
                .ident
                .clone()
                .map_or_else(|| syn::Member::Unnamed(index.into()), syn::Member::Named);
            let binding = format_ident!("field_{index}");
            let cfg = cfg_attrs(&field.attrs);
            quote!(#(#cfg)* #member: #binding,)
        });
        quote!({ #(#fields)* })
    }

    /// length of the fields bound by [`Self::pattern`]
    fn encoded_len(&self) -> TokenStream {
        let fields_len = self.fields_len();
        if self.frozen {
            fields_len
        } else {
            quote!(::ql_codec::encoded_len_prefixed(#fields_len))
        }
    }

    /// encodes the fields bound by [`Self::pattern`] into `out`
    fn encode(&self) -> TokenStream {
        let fields = self.shape.iter().enumerate().map(|(index, field)| {
            let binding = format_ident!("field_{index}");
            let cfg = cfg_attrs(&field.attrs);
            quote!(#(#cfg)* ::ql_codec::Encode::encode(#binding, out);)
        });
        let len = (!self.frozen).then(|| {
            let fields_len = self.fields_len();
            quote!(::ql_codec::encode_len_prefix(#fields_len, out);)
        });
        quote!(#len #(#fields)*)
    }

    fn fields_len(&self) -> TokenStream {
        let fields = self.shape.iter().enumerate().map(|(index, field)| {
            let binding = format_ident!("field_{index}");
            let cfg = cfg_attrs(&field.attrs);
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
        let input = self.input;
        self.construct(path, |field, _| {
            let ty = &field.ty;
            match (self.frozen, form) {
                (true, Form::Owned) => quote!(<#ty as ::ql_codec::Decode<#input>>::decode(reader)?),
                (true, Form::Borrowed) => {
                    quote!(<#ty as ::ql_codec::Decode<#input>>::decode_ref(reader)?)
                }
                (false, Form::Owned) => quote!(::ql_codec::decode_field::<#ty, _>(&mut fields)?),
                (false, Form::Borrowed) => {
                    quote!(::ql_codec::decode_field_ref::<#ty, _>(&mut fields)?)
                }
            }
        })
    }

    fn missing(&self, form: Form) -> TokenStream {
        let input = self.input;
        let value = self.construct(&form.path(), |field, _| {
            let ty = &field.ty;
            match form {
                Form::Owned => quote!(<#ty as ::ql_codec::Decode<#input>>::missing()?),
                Form::Borrowed => quote!(<#ty as ::ql_codec::Decode<#input>>::missing_ref()?),
            }
        });
        quote!(::core::option::Option::Some(#value))
    }

    /// builds the owned value from the borrowed fields bound by [`Self::pattern`]
    fn owned(&self, path: &TokenStream) -> TokenStream {
        let input = self.input;
        self.construct(path, |field, index| {
            let ty = &field.ty;
            let binding = format_ident!("field_{index}");
            quote!(<#ty as ::ql_codec::Decode<#input>>::from_ref(#binding))
        })
    }

    fn construct(
        &self,
        path: &TokenStream,
        value: impl Fn(&syn::Field, usize) -> TokenStream,
    ) -> TokenStream {
        let fields = self.shape.iter().enumerate().map(|(index, field)| {
            let member = field
                .ident
                .clone()
                .map_or_else(|| syn::Member::Unnamed(index.into()), syn::Member::Named);
            let cfg = cfg_attrs(&field.attrs);
            let value = value(field, index);
            quote!(#(#cfg)* #member: #value,)
        });
        quote!(#path { #(#fields)* })
    }

    /// the fields of the borrowed form in the declared shape
    fn ref_shape(&self) -> TokenStream {
        let input = self.input;
        let fields = self.shape.iter().map(|field| {
            let vis = &field.vis;
            let ty = &field.ty;
            let attrs = ref_attrs(&field.attrs);
            let name = field.ident.as_ref().map(|name| quote!(#name:));
            quote!(#(#attrs)* #vis #name <#ty as ::ql_codec::Decode<#input>>::Ref,)
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

/// the input container followed by the type's own generics
fn ref_generics(item: &syn::DeriveInput, input: &syn::Ident) -> syn::Generics {
    let mut generics = item.generics.clone();
    generics
        .params
        .insert(0, parse_quote!(#input: ::ql_codec::ByteSlice));
    if item.generics.type_params().next().is_none() {
        return generics;
    }
    with_field_bounds(
        &generics,
        item,
        |ty| parse_quote!(#ty: ::ql_codec::Decode<#input>),
    )
}

/// `generics` with `bound` on each field type when it has type parameters
fn with_field_bounds(
    generics: &syn::Generics,
    item: &syn::DeriveInput,
    bound: impl Fn(&syn::Type) -> syn::WherePredicate,
) -> syn::Generics {
    let mut generics = generics.clone();
    if generics.type_params().next().is_none() {
        return generics;
    }
    let predicates = &mut generics.make_where_clause().predicates;
    match &item.data {
        syn::Data::Struct(data) => {
            predicates.extend(data.fields.iter().map(|field| bound(&field.ty)))
        }
        syn::Data::Enum(data) => predicates.extend(
            data.variants
                .iter()
                .flat_map(|variant| &variant.fields)
                .map(|field| bound(&field.ty)),
        ),
        syn::Data::Union(_) => {}
    }
    generics
}

fn cfg_attrs(attrs: &[syn::Attribute]) -> impl Iterator<Item = &syn::Attribute> {
    attrs.iter().filter(|attr| attr.path().is_ident("cfg"))
}

/// Attributes carried over to the borrowed form
fn ref_attrs(attrs: &[syn::Attribute]) -> impl Iterator<Item = &syn::Attribute> {
    attrs
        .iter()
        .filter(|attr| attr.path().is_ident("cfg") || attr.path().is_ident("doc"))
}
