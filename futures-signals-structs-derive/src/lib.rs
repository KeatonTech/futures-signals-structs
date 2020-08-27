extern crate proc_macro;
extern crate proc_macro2;
extern crate syn;
#[macro_use]
extern crate quote;

use proc_macro::TokenStream;
use syn::{Field, Ident, ItemStruct, Type, Visibility};

/// Represents a field that needs to get converted to a Mutable and back.
enum MutableStructField {
    Basic {
        name: Ident,
        vis: Visibility,
        ty: Type,
    },
    MutableStruct {
        name: Ident,
        vis: Visibility,
        custom_mutable_ty: Ident,
    },
    Vec {
        name: Ident,
        vis: Visibility,
        inner_ty: Type,
    },
}

impl From<&Field> for MutableStructField {
    fn from(field: &Field) -> MutableStructField {
        if let Some(mutable_type_str) = MutableStructField::maybe_get_mutable_type(&field) {
            MutableStructField::MutableStruct {
                name: field.ident.clone().unwrap(),
                vis: field.vis.clone(),
                custom_mutable_ty: format_ident!("{}", mutable_type_str),
            }
        } else if let Some(vec_type) = MutableStructField::maybe_get_vec_type(&field) {
            MutableStructField::Vec {
                name: field.ident.clone().unwrap(),
                vis: field.vis.clone(),
                inner_ty: vec_type,
            }
        } else {
            MutableStructField::Basic {
                name: field.ident.clone().unwrap(),
                vis: field.vis.clone(),
                ty: field.ty.clone(),
            }
        }
    }
}

impl MutableStructField {
    /// Returns a struct definition of the mutable version of this field.
    pub fn get_mutable_field_definition(&self) -> proc_macro2::TokenStream {
        match self {
            MutableStructField::Basic { vis, name, ty } => {
                quote!(#vis #name: futures_signals::signal::Mutable<#ty>)
            }
            MutableStructField::MutableStruct {
                vis,
                name,
                custom_mutable_ty,
            } => quote!(#vis #name: #custom_mutable_ty),
            MutableStructField::Vec {
                vis,
                name,
                inner_ty,
            } => quote!(#vis #name: futures_signals::signal_vec::MutableVec<#inner_ty>),
        }
    }

    /// Returns code that can generate a constructor from a non-mutable version of the struct.
    pub fn get_constructor(&self, snapshot_name: Ident) -> proc_macro2::TokenStream {
        match self {
            MutableStructField::Basic { name, .. } => {
                quote!(futures_signals::signal::Mutable::new(#snapshot_name.#name))
            }
            MutableStructField::MutableStruct { name, .. } => {
                quote!(#snapshot_name.#name.as_mutable_struct())
            }
            MutableStructField::Vec { name, .. } => {
                quote!(futures_signals::signal_vec::MutableVec::new_with_values(#snapshot_name.#name.clone()))
            }
        }
    }

    /// Returns code that gets a static version of this field.
    pub fn get_snapshot_generator(&self) -> proc_macro2::TokenStream {
        match self {
            MutableStructField::Basic { name, .. } => quote!(self.#name.get_cloned()),
            MutableStructField::MutableStruct { name, .. } => quote!(self.#name.snapshot()),
            MutableStructField::Vec { name, .. } => {
                quote!(self.#name.lock_ref().as_slice().to_vec())
            }
        }
    }

    /// Returns code that updates the mutable value from a non-mutable version of this struct.
    pub fn get_update_setter(&self, snapshot_name: Ident) -> proc_macro2::TokenStream {
        match self {
            MutableStructField::Basic { name, .. } => quote!(self.#name.set(#snapshot_name.#name)),
            MutableStructField::MutableStruct { name, .. } => {
                quote!(self.#name.update(#snapshot_name.#name))
            }
            MutableStructField::Vec { name, .. } => {
                quote!(self.#name.lock_mut().replace_cloned(#snapshot_name.#name))
            }
        }
    }

    /// Returns the name of this field as an ident.
    pub fn get_name(&self) -> &proc_macro2::Ident {
        match self {
            MutableStructField::Basic { name, .. } => name,
            MutableStructField::MutableStruct { name, .. } => name,
            MutableStructField::Vec { name, .. } => name,
        }
    }

    /// Returns the value of the mutable_type annotation, if specified.
    fn maybe_get_mutable_type(input: &Field) -> Option<String> {
        for attr in &input.attrs {
            if !attr.path.is_ident("mutable_type") {
                continue;
            }
            if let Result::Ok(parsed_meta) = attr.parse_meta() {
                if let syn::Meta::NameValue(name_value) = parsed_meta {
                    if let syn::Lit::Str(lit_str) = name_value.lit {
                        return Some(lit_str.value());
                    } else {
                        panic!("Found a mutable_type that is not a string.")
                    }
                } else {
                    panic!("Format mutable_type as #[mutable_type = \"MyMutableName\"]")
                }
            } else {
                panic!("Found a malformed mutable_type. Format mutable_type as #[mutable_type = \"Name\"]");
            }
        }
        return Option::None;
    }

    /// Returns the generic type if this field is a Vec.
    fn maybe_get_vec_type(input: &Field) -> Option<Type> {
        if let syn::Type::Path(type_path) = &input.ty {
            let path_end = type_path.path.segments.last().unwrap();
            if path_end.ident == format_ident!("Vec") {
                if let syn::PathArguments::AngleBracketed(generic) = &path_end.arguments {
                    assert_eq!(generic.args.len(), 1);
                    if let syn::GenericArgument::Type(generic_type) = generic.args.last().unwrap() {
                        return Some(generic_type.clone());
                    } else {
                        panic!("Found a generic that is not a type");
                    }
                } else {
                    panic!("Found a Vec without a generic type");
                }
            }
        }
        return None;
    }
}

#[proc_macro_derive(AsMutableStruct, attributes(MutableStructName, mutable_type))]
pub fn as_mutable_struct(input: TokenStream) -> TokenStream {
    // Parse the string representation
    let ast: ItemStruct = syn::parse_macro_input!(input);

    // Determine what to name the Mutable version of this struct. Tries to pull from the
    // MutableStructName attribute, falls back to `MutableStructName` where StructName
    // is the name of the derived struct.
    let mutable_name = maybe_get_mutable_name(ast.clone())
        .map(|name| format_ident!("{}", name))
        .or(Some(format_ident!("Mutable{}", &ast.ident)))
        .unwrap();

    // Extract all fields as MutableStructField instances.
    let fields = ast.fields.iter().map(MutableStructField::from).collect();

    // Build the impl
    let gen_mutable = make_mutable_variant(ast.clone(), &fields, &mutable_name);
    let gen_as_signal_struct = impl_as_signal_struct(ast, &fields, &mutable_name);

    // Return the generated impl
    quote!(#gen_mutable #gen_as_signal_struct).into()
}

fn make_mutable_variant(
    input: ItemStruct,
    fields: &Vec<MutableStructField>,
    mutable_name: &Ident,
) -> proc_macro2::TokenStream {
    let original_ident = input.ident;
    let original_vis = input.vis;

    let mutable_fields = fields
        .iter()
        .map(MutableStructField::get_mutable_field_definition)
        .collect::<Vec<proc_macro2::TokenStream>>();

    let snapshot_fields = fields
        .iter()
        .map(|field| {
            let name = field.get_name();
            let snapshot_generator = field.get_snapshot_generator();
            quote!(#name: #snapshot_generator)
        })
        .collect::<Vec<proc_macro2::TokenStream>>();

    let update_fields = fields
        .iter()
        .map(|field| field.get_update_setter(format_ident!("new_snapshot")))
        .collect::<Vec<proc_macro2::TokenStream>>();

    quote! {
        #original_vis struct #mutable_name {
            #(#mutable_fields),*
        }

        impl futures_signals_structs_traits::MutableStruct for #mutable_name {
            type SnapshotType = #original_ident;

            fn snapshot(&self) -> #original_ident {
                #original_ident {
                    #(#snapshot_fields),*
                }
            }

            fn update(&self, new_snapshot: #original_ident) {
                #(#update_fields);*;
            }
        }

        impl Clone for #mutable_name {
            fn clone(&self) -> #mutable_name {
                self.snapshot().as_mutable_struct()
            }
        }
    }
}

fn impl_as_signal_struct(
    input: ItemStruct,
    fields: &Vec<MutableStructField>,
    mutable_name: &Ident,
) -> proc_macro2::TokenStream {
    let ident = input.ident;

    let mutable_fields = fields
        .iter()
        .map(|field| {
            let name = field.get_name();
            let mutable_constructor = field.get_constructor(format_ident!("self"));
            quote!(#name: #mutable_constructor)
        })
        .collect::<Vec<proc_macro2::TokenStream>>();

    quote! {
        impl futures_signals_structs_traits::AsMutableStruct for #ident {
            type MutableStructType = #mutable_name;

            fn as_mutable_struct(&self) -> #mutable_name {
                #mutable_name {
                    #(#mutable_fields),*
                }
            }
        }
    }
}

fn maybe_get_mutable_name(input: ItemStruct) -> Option<String> {
    for attr in input.attrs {
        if let syn::AttrStyle::Inner(_) = attr.style {
            continue;
        }
        if !attr.path.is_ident("MutableStructName") {
            continue;
        }
        if let Result::Ok(parsed_meta) = attr.parse_meta() {
            if let syn::Meta::NameValue(name_value) = parsed_meta {
                if let syn::Lit::Str(lit_str) = name_value.lit {
                    return Some(lit_str.value());
                } else {
                    panic!("Found a MutableStructName that is not a string.")
                }
            } else {
                panic!("Format MutableStructName as #[MutableStructName = \"MyMutableName\"]")
            }
        } else {
            panic!("Found a malformed MutableStructName. Format MutableStructName as #[MutableStructName = \"Name\"]");
        }
    }
    return Option::None;
}
