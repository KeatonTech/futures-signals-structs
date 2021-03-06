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
        ty: Type,
    },
}

impl From<&Field> for MutableStructField {
    fn from(field: &Field) -> MutableStructField {
        if MutableStructField::field_is_primitive(field) {
            MutableStructField::Basic {
                name: field.ident.clone().unwrap(),
                vis: field.vis.clone(),
                ty: field.ty.clone(),
            }
        } else {
            MutableStructField::MutableStruct {
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
                ty,
            } => quote!(#vis #name: <#ty as futures_signals_structs_traits::AsMutableStruct>::MutableStructType),
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
        }
    }

    /// Returns code that gets a static version of this field.
    pub fn get_snapshot_generator(&self) -> proc_macro2::TokenStream {
        match self {
            MutableStructField::Basic { name, .. } => quote!(self.#name.get_cloned()),
            MutableStructField::MutableStruct { name, .. } => quote!(self.#name.snapshot()),
        }
    }

    /// Returns code that updates the mutable value from a non-mutable version of this struct.
    pub fn get_update_setter(&self, snapshot_name: Ident) -> proc_macro2::TokenStream {
        match self {
            MutableStructField::Basic { name, .. } => quote!(self.#name.set(#snapshot_name.#name)),
            MutableStructField::MutableStruct { name, .. } => {
                quote!(self.#name.update(#snapshot_name.#name))
            }
        }
    }

    /// Returns the name of this field as an ident.
    pub fn get_name(&self) -> &proc_macro2::Ident {
        match self {
            MutableStructField::Basic { name, .. } => name,
            MutableStructField::MutableStruct { name, .. } => name,
        }
    }

    /// Returns the value of the mutable_type annotation, if specified.
    fn field_is_primitive(input: &Field) -> bool {
        if let Type::Path(type_path) = &input.ty {
            let last_component = type_path.path.segments.last().unwrap();
            let name = last_component.ident.to_string();
            name.chars().nth(0).unwrap().is_ascii_lowercase()
        } else {
            false
        }
    }
}

/// Derives a function called `as_mutable_struct()` that returns a version of the struct
/// where all fields are Mutable objects.
/// ```
///     #[derive(AsMutableStruct)]
///     struct PlayerScore {
///         hits: u32,
///         multiplier: f32,
///     }
/// 
///     fn main() {
///         let score = PlayerScore {
///             hits: 4,
///             multiplier: 0.4,
///         };
///         let mutable_score = score.as_mutable_struct();
///         mutable_score.hits.set(6);
///         mutable_score.multiplier.set(0.5);
///     }
/// ```
/// By default this creates a new struct called MutablePlayerScore that can also be
/// constructed directly as necessary.
/// ```
///     let mutable_score = MutablePlayerScore {
///         hits: Mutable::new(5),
///         multiplier: Mutable::new(1.4),
///     };
/// ```
/// Either way you construct it, the mutable object can be 'snapshotted' into the
/// original struct.
/// ```
///     assert_eq!(mutable_score.snapshot(), PlayerScore {
///         hits: 5,
///         multiplier: 1.4,
///     });
/// ```
/// The mutable value can also be updated to match a new static struct.
/// ```
///     mutable_score.update(PlayerScore {
///         hits: 50,
///         multiplier: 15,
///     });
///     assert_eq!(mutable_score.snapshot(), PlayerScore {
///         hits: 50,
///         multiplier: 15,
///     });
/// ```
/// Structs can depend on other structs when annotated with #[mutable_type]
/// ```
///     #[derive(AsMutableStruct)]
///     struct GameScore {
///         #[mutable_type = "MutablePlayerScore"] player_1: PlayerScore,
///         #[mutable_type = "MutablePlayerScore"] player_2: PlayerScore,
///     }
/// ```
#[proc_macro_derive(AsMutableStruct, attributes(MutableStructName))]
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
