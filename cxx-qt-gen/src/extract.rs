// SPDX-FileCopyrightText: 2021 Klarälvdalens Datakonsult AB, a KDAB Group company <info@kdab.com>
// SPDX-FileContributor: Andrew Hayzen <andrew.hayzen@kdab.com>
// SPDX-FileContributor: Gerhard de Clercq <gerhard.declercq@kdab.com>
//
// SPDX-License-Identifier: MIT OR Apache-2.0
use crate::utils::is_type_ident_ptr;
use convert_case::{Case, Casing};
use derivative::*;
use proc_macro2::{Span, TokenStream};
use std::result::Result;
use syn::{spanned::Spanned, *};

/// Describes an ident which has a different name in C++ and Rust
#[derive(Debug)]
pub(crate) struct CppRustIdent {
    /// The ident for C++
    pub(crate) cpp_ident: Ident,
    /// The ident for rust
    pub(crate) rust_ident: Ident,
}

/// Describes a Qt type
#[derive(Debug)]
pub enum QtTypes {
    I32,
    Pin {
        /// Cache of the last type_idents as a str for C++ to reference
        ident_str: String,
        /// Whether the inner type of this Pin is mut
        is_mut: bool,
        /// Whether the inner type of this Pin is the current type, and therefore "this" in C++
        is_this: bool,
        /// The ident of the inner type (eg the T of Pin<T>)
        type_idents: Vec<Ident>,
    },
    Ptr {
        /// Cache of the last type ident as a str for C++ to reference
        ident_str: String,
    },
    // TODO: these will become QString in the future
    String,
    Str,
}

/// Describes a type
#[derive(Debug)]
pub(crate) struct ParameterType {
    /// The type of the parameter
    pub(crate) idents: Vec<Ident>,
    /// If this parameter is a reference
    pub(crate) is_ref: bool,
    /// The original type, this allows us to annotate an error with a span later
    pub(crate) original_ty: syn::Type,
    /// The detected Qt type of the parameter
    pub(crate) qt_type: QtTypes,
}

/// Describes a function parameter
#[derive(Debug)]
pub(crate) struct Parameter {
    /// The ident of the parameter
    pub(crate) ident: Ident,
    /// The type of the parameter
    pub(crate) type_ident: ParameterType,
}

/// Describes a function that can be invoked from QML
#[derive(Derivative)]
#[derivative(Debug)]
pub(crate) struct Invokable {
    /// The ident of the function
    pub(crate) ident: CppRustIdent,
    /// The parameters that the function takes in
    pub(crate) parameters: Vec<Parameter>,
    /// The return type information
    pub(crate) return_type: Option<ParameterType>,
    /// The original Rust method for the invokable
    #[derivative(Debug = "ignore")]
    pub(crate) original_method: ImplItemMethod,
}

/// Describes a property that can be used from QML
#[derive(Debug)]
pub(crate) struct Property {
    /// The ident of the property
    pub(crate) ident: CppRustIdent,
    /// The type of the property
    pub(crate) type_ident: ParameterType,
    /// The getter ident of the property (used for READ)
    pub(crate) getter: Option<CppRustIdent>,
    /// The setter ident of the property (used for WRITE)
    pub(crate) setter: Option<CppRustIdent>,
    /// The notify ident of the property (used for NOTIFY)
    pub(crate) notify: Option<CppRustIdent>,
    // TODO: later we will further possibilities such as CONSTANT or FINAL
}

/// Describes all the properties of a QObject class
#[derive(Debug)]
pub struct QObject {
    /// The ident of the original struct and name of the C++ class that represents the QObject
    pub ident: Ident,
    /// The ident of the new Rust struct that will be generated and will form the internals of the QObject
    pub(crate) rust_struct_ident: Ident,
    /// The ident of the new Rust wrapper that will be generated and will provide a nice interface to the CppObject
    pub(crate) rust_wrapper_ident: Ident,
    /// All the methods that can be invoked from QML
    pub(crate) invokables: Vec<Invokable>,
    /// All the properties that can be used from QML
    pub(crate) properties: Vec<Property>,
    /// The original Rust mod for the struct
    pub(crate) original_mod: ItemMod,
    /// The original Rust struct that the object was generated from
    pub(crate) original_struct: ItemStruct,
    /// The original Rust trait impls for the struct
    pub(crate) original_trait_impls: Vec<ItemImpl>,
    /// The original Rust use declarations from the mod
    pub(crate) original_use_decls: Vec<ItemUse>,
}

/// Describe the error type from extract_qt_type and extract_type_ident
enum ExtractTypeIdentError {
    /// We do not support AngleBracketed or Parenthesized rust types
    InvalidArguments(Span),
    /// This is not a valid rust type
    InvalidType(Span),
    /// There are no idents in the type
    IdentEmpty(Span),
    /// There are multiple idents but didn't start with crate::
    UnknownAndNotCrate(Span),
    /// There is one ident but it's unknown to our converters
    UnknownIdent(Span),
    /// There is a Pin<T> but the T is unknown to our converters
    UnknownPinType(Span),
}

/// Extract the Qt type from a list of Ident's
fn extract_qt_type(
    idents: &[Ident],
    original_ty: &syn::Type,
) -> Result<QtTypes, ExtractTypeIdentError> {
    // TODO: can we support generic Qt types as well eg like QObject or QAbstractListModel?
    // so that QML can set a C++/QML type into the property ? or is that not useful?

    // Check that the type has at least one ident
    if idents.is_empty() {
        Err(ExtractTypeIdentError::IdentEmpty(original_ty.span()))
    // If there is one entry then try to convert using our defined types
    } else if idents.len() == 1 {
        // We can assume that idents has an entry at index zero, because there is one entry
        match idents[0].to_string().as_str() {
            // TODO: these will become QString in the future
            "str" => Ok(QtTypes::Str),
            "String" => Ok(QtTypes::String),
            "i32" => Ok(QtTypes::I32),
            _other => Err(ExtractTypeIdentError::UnknownIdent(idents[0].span())),
        }
    // As this type ident has more than one segment, check if it is a pointer
    } else if is_type_ident_ptr(idents) {
        Ok(QtTypes::Ptr {
            // TODO: on the C++ side we only have the last segment always? crate::sub_object::SubObject -> SubObject?
            // maybe if we do namespacing this will then become important?
            // ident_str: idents
            //     .iter()
            //     .map(|ident| ident.to_string())
            //     .collect::<Vec<String>>()
            //     .join("::"),
            //
            // TODO: do we need to track is_ref here?
            //
            // We can assume that last exists as there is at least one entry in idents, so unwrap() is fine here
            ident_str: idents.last().unwrap().to_string(),
        })
    // This is an unknown type that did not start with crate and has multiple parts
    } else {
        // We can assume that idents has an entry at index zero, because it is not empty
        Err(ExtractTypeIdentError::UnknownAndNotCrate(idents[0].span()))
    }
}

/// Converts a given path to a vector of idents
fn path_to_idents(path: &syn::Path) -> Result<Vec<Ident>, ExtractTypeIdentError> {
    path.segments
        .iter()
        .map(|segment| {
            // We do not support PathArguments for types in properties or arguments
            //
            // eg we do not support AngleBracketed - the <'a, T> in std::slice::iter<'a, T>
            // eg we do not support Parenthesized - the (A, B) -> C in Fn(A, B) -> C
            if segment.arguments == PathArguments::None {
                Ok(segment.ident.to_owned())
            } else {
                Err(ExtractTypeIdentError::InvalidArguments(segment.span()))
            }
        })
        .collect::<Result<Vec<Ident>, ExtractTypeIdentError>>()
}

/// Extract the type ident from a given syn::Type
fn extract_type_ident(ty: &syn::Type) -> Result<ParameterType, ExtractTypeIdentError> {
    // Temporary storage of the current syn::TypePath if one is found
    let ty_path;
    // Whether this syn::Type is a reference or not
    let is_ref;

    match ty {
        // The type is simply a path (eg std::slice::Iter)
        Type::Path(path) => {
            is_ref = false;
            ty_path = path;
        }
        // The type is a reference, so see if it contains a path
        Type::Reference(TypeReference { elem, .. }) => {
            // If the type is a path then extract it and mark is_ref
            if let Type::Path(path) = &**elem {
                is_ref = true;
                ty_path = path;
            } else {
                return Err(ExtractTypeIdentError::InvalidType(ty.span()));
            }
        }
        _others => {
            return Err(ExtractTypeIdentError::InvalidType(ty.span()));
        }
    }

    // Check if this type is a Pin<T>, if it is then attempt to extract it
    if let Some(segment) = ty_path.path.segments.first() {
        if segment.ident.to_string().as_str() == "Pin" {
            match &segment.arguments {
                PathArguments::AngleBracketed(AngleBracketedGenericArguments { args, .. })
                    if args.len() == 1 =>
                {
                    let is_mut;
                    let ty_path;

                    // We have already checked that args is of len 1
                    match &args[0] {
                        // We are &mut T
                        GenericArgument::Type(Type::Reference(TypeReference {
                            elem,
                            mutability,
                            ..
                        })) => {
                            is_mut = mutability.is_some();

                            if let Type::Path(path) = &**elem {
                                ty_path = path;
                            } else {
                                return Err(ExtractTypeIdentError::UnknownPinType(ty.span()));
                            }
                        }
                        // TODO: later we might want extra cases to handle non ref versions? Pin<T>
                        _others => {
                            return Err(ExtractTypeIdentError::UnknownPinType(ty.span()));
                        }
                    }

                    // Convert our inner type to a list of idents
                    let type_idents = path_to_idents(&ty_path.path)?;
                    // Create the Qt type for the Pin
                    let qt_type = QtTypes::Pin {
                        ident_str: if let Some(ident) = type_idents.last() {
                            ident.to_string()
                        } else {
                            // There was no T part of Pin<T>
                            //
                            // TODO: could be it's own enum error? InvalidPinType?
                            return Err(ExtractTypeIdentError::UnknownPinType(ty.span()));
                        },
                        is_mut,
                        // If the T in Pin<T> is CppObj, then it is "this"
                        is_this: if let Some(ident) = type_idents.first() {
                            ident.to_string().as_str() == "CppObj"
                        } else {
                            false
                        },
                        type_idents,
                    };
                    // We put the Pin as our idents, the inner type goes into the QtType
                    //
                    // The gen_cpp and gen_rs don't use this as they'll special case Pin<T> when
                    // it's an invokable argument to use the inner type.
                    // It is only used in return types or Q_PROPERTY in gen_rs
                    //
                    // TODO: should Pin<T> be accepted for return types and Q_PROPERTY?
                    let idents = vec![segment.ident.to_owned()];

                    return Ok(ParameterType {
                        // Read each of the path segment to turn a &syn::TypePath of std::slice::Iter
                        // into an owned Vec<Ident>
                        idents,
                        is_ref,
                        // We need to have the original type so that errors can Span if there are no idents
                        original_ty: ty.to_owned(),
                        qt_type,
                    });
                }
                _ => {}
            }
        }
    }

    let idents = path_to_idents(&ty_path.path)?;
    // Extract the Qt type this is used in C++ and Rust generation
    let qt_type = extract_qt_type(&idents, ty)?;

    // Create and return a ParameterType
    Ok(ParameterType {
        // Read each of the path segment to turn a &syn::TypePath of std::slice::Iter
        // into an owned Vec<Ident>
        idents,
        is_ref,
        // We need to have the original type so that errors can Span if there are no idents
        original_ty: ty.to_owned(),
        qt_type,
    })
}

/// Extracts all the member functions from a module and generates invokables from them
fn extract_invokables(items: &[ImplItem]) -> Result<Vec<Invokable>, TokenStream> {
    let mut invokables = Vec::new();

    // TODO: we need to set up an exclude list of invokable names and give
    // the user an error if they use one of those names.
    // This is to avoid name collisions with CxxQObject standard functions.

    // Process each impl item and turn into an Invokable or error
    for item in items {
        // Check if this item is a method
        //
        // TODO: later should we pass through unknown items
        // or should they have an attribute to ignore
        let method;
        if let ImplItem::Method(m) = item {
            method = m;
        } else {
            return Err(Error::new(item.span(), "Only methods are supported.").to_compile_error());
        }

        // Extract the ident, parameters, return type of the method
        let method_ident = &method.sig.ident;
        let inputs = &method.sig.inputs;
        let output = &method.sig.output;

        // Prepare a vector to store the processed parameters of the method
        let mut parameters = Vec::new();

        // Process each input (parameters) of the method adding Parameter's to parameters
        for parameter in inputs {
            // Check that the parameter is typed
            //
            // If it is not typed (it is a syn::Receiver) then this means it is the self parameter
            // but without a type, eg self: Box<Self> would be Typed
            //
            // TODO: does this mean that if self is Typed we need to skip it?
            // so should we ignore the first parameter if it is named "self"?
            if let FnArg::Typed(PatType { pat, ty, .. }) = parameter {
                // The name ident of the parameter
                let parameter_ident;
                // The type ident of the parameter
                let type_ident;

                // Try to extract the name of the parameter
                if let Pat::Ident(PatIdent { ident, .. }) = &**pat {
                    parameter_ident = ident;
                } else {
                    return Err(
                        Error::new(parameter.span(), "Invalid argument ident format.")
                            .to_compile_error(),
                    );
                }

                // Try to extract the type of the parameter
                match extract_type_ident(ty) {
                    Ok(result) => type_ident = result,
                    Err(ExtractTypeIdentError::InvalidArguments(span)) => {
                        return Err(Error::new(
                            span,
                            "Argument should not be angle bracketed or parenthesized.",
                        )
                        .to_compile_error());
                    }
                    Err(ExtractTypeIdentError::InvalidType(span)) => {
                        return Err(
                            Error::new(span, "Invalid argument ident format.").to_compile_error()
                        )
                    }
                    Err(ExtractTypeIdentError::IdentEmpty(span)) => {
                        return Err(Error::new(span, "Argument type ident must have at least one segment").to_compile_error())
                    }
                    Err(ExtractTypeIdentError::UnknownAndNotCrate(span)) => {
                        return Err(Error::new(span, "First argument type ident segment must start with 'crate' if there are multiple").to_compile_error())
                    }
                    Err(ExtractTypeIdentError::UnknownIdent(span)) => {
                        return Err(Error::new(span, "Unknown argument type ident to parse").to_compile_error())
                    }
                    Err(ExtractTypeIdentError::UnknownPinType(span)) => {
                        return Err(Error::new(span, "Unknown argument Pin<T> type ident to parse").to_compile_error())
                    }
                }

                // Build and push the parameter
                parameters.push(Parameter {
                    ident: parameter_ident.to_owned(),
                    type_ident,
                });
            }
        }

        // Process the output and determine if it has a return type
        let return_type = if let ReturnType::Type(_, ty) = output {
            // This output has a return type, so extract the type
            match extract_type_ident(ty) {
                Ok(result) => Some(result),
                Err(ExtractTypeIdentError::InvalidArguments(span)) => {
                    return Err(Error::new(
                        span,
                        "Return type should not be angle bracketed or parenthesized.",
                    )
                    .to_compile_error());
                }
                Err(ExtractTypeIdentError::InvalidType(span)) => {
                    return Err(Error::new(span, "Invalid return type format.").to_compile_error())
                }
                Err(ExtractTypeIdentError::IdentEmpty(span)) => {
                    return Err(Error::new(
                        span,
                        "Return type ident must have at least one segment",
                    )
                    .to_compile_error())
                }
                Err(ExtractTypeIdentError::UnknownAndNotCrate(span)) => return Err(Error::new(
                    span,
                    "First return type ident segment must start with 'crate' if there are multiple",
                )
                .to_compile_error()),
                Err(ExtractTypeIdentError::UnknownIdent(span)) => {
                    return Err(
                        Error::new(span, "Unknown return type ident to parse").to_compile_error()
                    )
                }
                Err(ExtractTypeIdentError::UnknownPinType(span)) => {
                    return Err(
                        Error::new(span, "Unknown return Pin<T> type ident to parse")
                            .to_compile_error(),
                    )
                }
            }
        } else {
            None
        };

        // TODO: later support an attribute to keep original or override renaming
        let ident_str = method_ident.to_string();
        let ident_method = CppRustIdent {
            cpp_ident: quote::format_ident!("{}", ident_str.to_case(Case::Camel)),
            rust_ident: quote::format_ident!("{}", ident_str.to_case(Case::Snake)),
        };

        // Build and push the invokable
        let invokable = Invokable {
            ident: ident_method,
            parameters,
            return_type,
            original_method: method.to_owned(),
        };
        invokables.push(invokable);
    }

    Ok(invokables)
}

/// Extracts all the attributes from a struct and generates properties from them
fn extract_properties(s: &ItemStruct) -> Result<Vec<Property>, TokenStream> {
    let mut properties = Vec::new();

    // TODO: we need to set up an exclude list of properties names and give
    // the user an error if they use one of those names.
    // For instance "rustObj" is not allowed as that would cause a collision.

    // Read the properties from the struct
    //
    // Extract only the named fields (eg "Point { x: f64, y: f64 }") and ignore any
    // unnamed fields (eg "Some(T)") or units (eg "None")
    if let ItemStruct {
        fields: Fields::Named(FieldsNamed { named, .. }),
        ..
    } = s
    {
        // Process each named field individually
        for name in named {
            // Extract only fields with an ident (should be all as these are named fields).
            if let Field {
                // TODO: later we'll need to read the attributes (eg qt_property) here
                // attrs,
                ident: Some(ident),
                ty,
                ..
            } = name
            {
                // Extract the type of the field
                let type_ident;

                match extract_type_ident(ty) {
                    Ok(result) => type_ident = result,
                    Err(ExtractTypeIdentError::InvalidArguments(span)) => {
                        return Err(Error::new(
                            span,
                            "Named field should not be angle bracketed or parenthesized.",
                        )
                        .to_compile_error());
                    }
                    Err(ExtractTypeIdentError::InvalidType(span)) => {
                        return Err(
                            Error::new(span, "Invalid name field ident format.").to_compile_error()
                        )
                    }
                    Err(ExtractTypeIdentError::IdentEmpty(span)) => {
                        return Err(Error::new(span, "Named field type ident must have at least one segment").to_compile_error())
                    }
                    Err(ExtractTypeIdentError::UnknownAndNotCrate(span)) => {
                        return Err(Error::new(span, "First named field type ident segment must start with 'crate' if there are multiple").to_compile_error())
                    }
                    Err(ExtractTypeIdentError::UnknownIdent(span)) => {
                        return Err(Error::new(span, "Unknown named field type ident to parse").to_compile_error())
                    }
                    Err(ExtractTypeIdentError::UnknownPinType(span)) => {
                        return Err(Error::new(span, "Unknown named field Pin<T> type ident to parse").to_compile_error())
                    }
                }

                // Build the getter/setter/notify idents with their Rust and C++ idents
                //
                // TODO: later these can be optional and have custom names from macro attributes
                //
                // TODO: we might also need to store whether a custom method is already implemented
                // or whether a method needs to be auto generated on the rust side
                //
                // TODO: later support an attribute to keep original or override renaming
                let ident_str = ident.to_string();
                let ident_prop = CppRustIdent {
                    cpp_ident: quote::format_ident!("{}", ident_str.to_case(Case::Camel)),
                    rust_ident: quote::format_ident!("{}", ident_str.to_case(Case::Snake)),
                };
                let getter = Some(CppRustIdent {
                    cpp_ident: quote::format_ident!("get{}", ident_str.to_case(Case::Pascal)),
                    rust_ident: quote::format_ident!("{}", ident_str.to_case(Case::Snake)),
                });
                let setter = Some(CppRustIdent {
                    cpp_ident: quote::format_ident!("set{}", ident_str.to_case(Case::Pascal)),
                    rust_ident: quote::format_ident!("set_{}", ident_str.to_case(Case::Snake)),
                });
                let notify = Some(CppRustIdent {
                    cpp_ident: quote::format_ident!("{}Changed", ident_str.to_case(Case::Camel)),
                    // TODO: rust doesn't have notify on it's side?
                    rust_ident: quote::format_ident!("{}", ident_str.to_case(Case::Snake)),
                });

                // Build and push the property
                properties.push(Property {
                    ident: ident_prop,
                    type_ident,
                    getter,
                    setter,
                    notify,
                });
            }
        }
    }

    Ok(properties)
}

/// Parses a module in order to extract a QObject description from it
pub fn extract_qobject(module: ItemMod) -> Result<QObject, TokenStream> {
    // Static internal rust suffix name
    const RUST_SUFFIX: &str = "Rs";
    const RUST_WRAPPER_SUFFIX: &str = "Wrapper";

    // Find the items from the module
    let original_mod = module.to_owned();
    let items = &mut module
        .content
        .expect("Incorrect module format encountered.")
        .1;

    // Prepare variables to store struct, invokables, and other data
    //
    // The original Item::Struct if one is found
    let mut original_struct = None;
    // The name of the struct if one was found
    let mut struct_ident = None;
    // The name we will use for the rust generated struct if we find one
    let mut rust_struct_ident = None;
    // The name we will use for the rust generated wrapper if we find one
    let mut rust_wrapper_ident = None;

    // A list of the invokables for the struct
    let mut object_invokables = vec![];
    // A list of original trait impls for the struct (eg impl Default for Struct)
    let mut original_trait_impls = vec![];
    // A list of original use declarations for the mod (eg use crate::thing)
    let mut original_use_decls = vec![];

    // Process each of the items in the mod
    for item in items.drain(..) {
        match item {
            // We are a struct
            Item::Struct(s) => {
                // Check that we are the first struct
                if original_struct.is_none() {
                    // Make a copy of the ident
                    struct_ident = Some(s.ident.to_owned());
                    // Move the original struct
                    original_struct = Some(s);
                    // Build rust versions of the struct ident
                    rust_struct_ident = Some(quote::format_ident!(
                        "{}{}",
                        struct_ident.as_ref().unwrap(),
                        RUST_SUFFIX
                    ));
                    rust_wrapper_ident = Some(quote::format_ident!(
                        "{}{}",
                        struct_ident.as_ref().unwrap(),
                        RUST_WRAPPER_SUFFIX
                    ));
                } else {
                    return Err(
                        Error::new(s.span(), "Only one struct is supported per mod.")
                            .to_compile_error(),
                    );
                }
            }
            // We are an impl
            Item::Impl(mut original_impl) => {
                // Ensure that the struct block has already happened
                if original_struct.is_none() {
                    return Err(Error::new(
                        original_impl.span(),
                        "Impl can only be declared after a struct.",
                    )
                    .to_compile_error());
                }

                // Extract the path from the type (this leads to the struct name)
                if let Type::Path(TypePath { path, .. }) = &mut *original_impl.self_ty {
                    // Check that the path contains segments
                    if path.segments.len() != 1 {
                        return Err(Error::new(
                            original_impl.span(),
                            "Invalid path on impl block.",
                        )
                        .to_compile_error());
                    }

                    // Retrieve the impl struct name and check it's the same as the declared struct
                    //
                    // We can assume that segments[0] works as we have checked length to be 1
                    let impl_ident = &path.segments[0].ident;
                    // We can assume that struct_ident exists as we checked there was a struct
                    if impl_ident != struct_ident.as_ref().unwrap() {
                        return Err(Error::new(
                            impl_ident.span(),
                            "The impl block needs to match the struct.",
                        )
                        .to_compile_error());
                    }

                    // Check if this impl is a impl or impl Trait
                    if original_impl.trait_.is_none() {
                        // Add invokables if this is just an impl block
                        object_invokables.append(&mut extract_invokables(&original_impl.items)?);
                    } else {
                        // We are a impl trait so rename the struct and add to vec
                        // We can assume that segments[0] works as we have checked length to be 1
                        let impl_ident = &mut path.segments[0].ident;
                        // We can assume that struct_ident exists as we checked there was a struct
                        if impl_ident == struct_ident.as_ref().unwrap() {
                            // Rename the ident of the struct
                            *impl_ident = quote::format_ident!("{}{}", impl_ident, RUST_SUFFIX);
                            original_trait_impls.push(original_impl.to_owned());
                        } else {
                            return Err(Error::new(
                                impl_ident.span(),
                                "The impl Trait block needs to match the struct.",
                            )
                            .to_compile_error());
                        }
                    }
                } else {
                    return Err(Error::new(
                        original_impl.span(),
                        "Expected a TypePath impl to parse.",
                    )
                    .to_compile_error());
                }
            }
            // We are a use so pass to use declaration list
            Item::Use(u) => {
                original_use_decls.push(u.to_owned());
            }
            // TODO: consider what other items we allow in the mod, we may just pass through all
            // the remaining types as an unknown list which the gen side can put at the end?
            // Are all the remaining types safe to pass through or do we need to exclude any?
            other => {
                return Err(Error::new(other.span(), "Unsupported item in mod.").to_compile_error());
            }
        }
    }

    // Check that we found a struct
    if original_struct.is_none() {
        panic!("There must be at least one struct per mod");
    }
    let original_struct = original_struct.unwrap();

    // Read properties from the struct
    let object_properties = extract_properties(&original_struct)?;

    Ok(QObject {
        ident: struct_ident.unwrap(),
        rust_struct_ident: rust_struct_ident.unwrap(),
        rust_wrapper_ident: rust_wrapper_ident.unwrap(),
        invokables: object_invokables,
        properties: object_properties,
        original_mod,
        original_struct,
        original_trait_impls,
        original_use_decls,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    use pretty_assertions::assert_eq;

    #[test]
    fn parses_basic_custom_default() {
        // TODO: we probably want to parse all the test case files we have
        // only once as to not slow down different tests on the same input.
        // This can maybe be done with some kind of static object somewhere.
        let source = include_str!("../test_inputs/basic_custom_default.rs");
        let module: ItemMod = syn::parse_str(source).unwrap();
        let qobject = extract_qobject(module).unwrap();

        // Check that it got the inovkables and properties
        assert_eq!(qobject.invokables.len(), 1);
        assert_eq!(qobject.properties.len(), 1);

        // Check that impl Default was found
        assert_eq!(qobject.original_trait_impls.len(), 1);
        let trait_impl = &qobject.original_trait_impls[0];
        if let Type::Path(TypePath { path, .. }) = &*trait_impl.self_ty {
            assert_eq!(path.segments.len(), 1);
            assert_eq!(path.segments[0].ident.to_string(), "MyObjectRs");
        } else {
            panic!("Trait impl was not a TypePath");
        }
    }

    #[test]
    fn parses_basic_ident_changes() {
        // TODO: we probably want to parse all the test case files we have
        // only once as to not slow down different tests on the same input.
        // This can maybe be done with some kind of static object somewhere.
        let source = include_str!("../test_inputs/basic_ident_changes.rs");
        let module: ItemMod = syn::parse_str(source).unwrap();
        let qobject = extract_qobject(module).unwrap();

        // Check that it got the properties and that the idents are correct
        assert_eq!(qobject.properties.len(), 1);

        // Check first property
        let prop_first = &qobject.properties[0];
        assert_eq!(prop_first.ident.cpp_ident.to_string(), "myNumber");
        assert_eq!(prop_first.ident.rust_ident.to_string(), "my_number");
        assert_eq!(prop_first.type_ident.idents.len(), 1);
        assert_eq!(prop_first.type_ident.idents[0].to_string(), "i32");
        assert_eq!(prop_first.type_ident.is_ref, false);

        assert_eq!(prop_first.getter.is_some(), true);
        let getter = prop_first.getter.as_ref().unwrap();
        assert_eq!(getter.cpp_ident.to_string(), "getMyNumber");
        assert_eq!(getter.rust_ident.to_string(), "my_number");

        assert_eq!(prop_first.setter.is_some(), true);
        let setter = prop_first.setter.as_ref().unwrap();
        assert_eq!(setter.cpp_ident.to_string(), "setMyNumber");
        assert_eq!(setter.rust_ident.to_string(), "set_my_number");

        assert_eq!(prop_first.notify.is_some(), true);
        let notify = prop_first.notify.as_ref().unwrap();
        assert_eq!(notify.cpp_ident.to_string(), "myNumberChanged");
        // TODO: does rust need a notify ident?
        assert_eq!(notify.rust_ident.to_string(), "my_number");

        // Check that it got the invokables
        assert_eq!(qobject.invokables.len(), 1);

        // Check invokable ident
        let invokable = &qobject.invokables[0];
        assert_eq!(invokable.ident.cpp_ident.to_string(), "sayBye");
        assert_eq!(invokable.ident.rust_ident.to_string(), "say_bye");

        // Check invokable parameters ident and type ident
        assert_eq!(invokable.parameters.len(), 0);
    }

    #[test]
    fn parses_basic_invokable_and_properties() {
        // TODO: we probably want to parse all the test case files we have
        // only once as to not slow down different tests on the same input.
        // This can maybe be done with some kind of static object somewhere.
        let source = include_str!("../test_inputs/basic_invokable_and_properties.rs");
        let module: ItemMod = syn::parse_str(source).unwrap();
        let qobject = extract_qobject(module).unwrap();

        // Check that it got the invokables and properties
        // We only check the counts as the only_invokables and only_properties
        // will test more than the number.
        assert_eq!(qobject.invokables.len(), 2);
        assert_eq!(qobject.properties.len(), 2);
    }

    #[test]
    fn parses_basic_only_invokable() {
        // TODO: we probably want to parse all the test case files we have
        // only once as to not slow down different tests on the same input.
        // This can maybe be done with some kind of static object somewhere.
        let source = include_str!("../test_inputs/basic_only_invokable.rs");
        let module: ItemMod = syn::parse_str(source).unwrap();
        let qobject = extract_qobject(module).unwrap();

        // Check that it got the names right
        assert_eq!(qobject.ident.to_string(), "MyObject");
        assert_eq!(qobject.original_mod.ident.to_string(), "my_object");
        assert_eq!(qobject.rust_struct_ident.to_string(), "MyObjectRs");

        // Check that it got the invokables
        assert_eq!(qobject.invokables.len(), 2);

        // Check invokable ident
        let invokable = &qobject.invokables[0];
        assert_eq!(invokable.ident.cpp_ident.to_string(), "sayHi");
        assert_eq!(invokable.ident.rust_ident.to_string(), "say_hi");

        // Check invokable parameters ident and type ident
        assert_eq!(invokable.parameters.len(), 2);

        let param_first = &invokable.parameters[0];
        assert_eq!(param_first.ident.to_string(), "string");
        // TODO: add extra checks when we read if this is a mut or not
        assert_eq!(param_first.type_ident.idents.len(), 1);
        assert_eq!(param_first.type_ident.idents[0].to_string(), "str");
        assert_eq!(param_first.type_ident.is_ref, true);

        let param_second = &invokable.parameters[1];
        assert_eq!(param_second.ident.to_string(), "number");
        assert_eq!(param_second.type_ident.idents.len(), 1);
        assert_eq!(param_second.type_ident.idents[0].to_string(), "i32");
        assert_eq!(param_second.type_ident.is_ref, false);

        // Check invokable ident
        let invokable_second = &qobject.invokables[1];
        assert_eq!(invokable_second.ident.cpp_ident.to_string(), "sayBye");
        assert_eq!(invokable_second.ident.rust_ident.to_string(), "say_bye");

        // Check invokable parameters ident and type ident
        assert_eq!(invokable_second.parameters.len(), 0);
    }

    #[test]
    fn parses_basic_only_properties() {
        // TODO: we probably want to parse all the test case files we have
        // only once as to not slow down different tests on the same input.
        // This can maybe be done with some kind of static object somewhere.
        let source = include_str!("../test_inputs/basic_only_properties.rs");
        let module: ItemMod = syn::parse_str(source).unwrap();
        let qobject = extract_qobject(module).unwrap();

        // Check that it got the properties and that the idents are correct
        assert_eq!(qobject.properties.len(), 2);

        // Check first property
        let prop_first = &qobject.properties[0];
        assert_eq!(prop_first.ident.cpp_ident.to_string(), "number");
        assert_eq!(prop_first.ident.rust_ident.to_string(), "number");
        assert_eq!(prop_first.type_ident.idents.len(), 1);
        assert_eq!(prop_first.type_ident.idents[0].to_string(), "i32");
        assert_eq!(prop_first.type_ident.is_ref, false);

        assert_eq!(prop_first.getter.is_some(), true);
        let getter = prop_first.getter.as_ref().unwrap();
        assert_eq!(getter.cpp_ident.to_string(), "getNumber");
        assert_eq!(getter.rust_ident.to_string(), "number");

        assert_eq!(prop_first.setter.is_some(), true);
        let setter = prop_first.setter.as_ref().unwrap();
        assert_eq!(setter.cpp_ident.to_string(), "setNumber");
        assert_eq!(setter.rust_ident.to_string(), "set_number");

        assert_eq!(prop_first.notify.is_some(), true);
        let notify = prop_first.notify.as_ref().unwrap();
        assert_eq!(notify.cpp_ident.to_string(), "numberChanged");
        // TODO: does rust need a notify ident?
        assert_eq!(notify.rust_ident.to_string(), "number");

        // Check second property
        let prop_second = &qobject.properties[1];
        assert_eq!(prop_second.ident.cpp_ident.to_string(), "string");
        assert_eq!(prop_second.ident.rust_ident.to_string(), "string");
        assert_eq!(prop_second.type_ident.idents.len(), 1);
        assert_eq!(prop_second.type_ident.idents[0].to_string(), "String");
        assert_eq!(prop_second.type_ident.is_ref, false);

        assert_eq!(prop_second.getter.is_some(), true);
        let getter = prop_second.getter.as_ref().unwrap();
        assert_eq!(getter.cpp_ident.to_string(), "getString");
        assert_eq!(getter.rust_ident.to_string(), "string");

        assert_eq!(prop_second.setter.is_some(), true);
        let setter = prop_second.setter.as_ref().unwrap();
        assert_eq!(setter.cpp_ident.to_string(), "setString");
        assert_eq!(setter.rust_ident.to_string(), "set_string");

        assert_eq!(prop_second.notify.is_some(), true);
        let notify = prop_second.notify.as_ref().unwrap();
        assert_eq!(notify.cpp_ident.to_string(), "stringChanged");
        // TODO: does rust need a notify ident?
        assert_eq!(notify.rust_ident.to_string(), "string");
    }

    #[test]
    fn parses_basic_mod_use() {
        // TODO: we probably want to parse all the test case files we have
        // only once as to not slow down different tests on the same input.
        // This can maybe be done with some kind of static object somewhere.
        let source = include_str!("../test_inputs/basic_mod_use.rs");
        let module: ItemMod = syn::parse_str(source).unwrap();
        let qobject = extract_qobject(module).unwrap();

        // Check that it got the inovkables and properties
        assert_eq!(qobject.invokables.len(), 1);
        assert_eq!(qobject.properties.len(), 1);

        // Check that there is a use declaration
        assert_eq!(qobject.original_use_decls.len(), 1);
    }

    #[test]
    fn parses_basic_pin_invokable() {
        // TODO: we probably want to parse all the test case files we have
        // only once as to not slow down different tests on the same input.
        // This can maybe be done with some kind of static object somewhere.
        let source = include_str!("../test_inputs/basic_pin_invokable.rs");
        let module: ItemMod = syn::parse_str(source).unwrap();
        let qobject = extract_qobject(module).unwrap();

        // Check that it got the names right
        assert_eq!(qobject.ident.to_string(), "MyObject");
        assert_eq!(qobject.original_mod.ident.to_string(), "my_object");
        assert_eq!(qobject.rust_struct_ident.to_string(), "MyObjectRs");

        // Check that it got the invokables
        assert_eq!(qobject.invokables.len(), 2);

        // Check invokable ident
        let invokable = &qobject.invokables[0];
        assert_eq!(invokable.ident.cpp_ident.to_string(), "sayHi");
        assert_eq!(invokable.ident.rust_ident.to_string(), "say_hi");

        // Check invokable parameters ident and type ident
        assert_eq!(invokable.parameters.len(), 3);

        let param_first = &invokable.parameters[0];
        assert_eq!(param_first.ident.to_string(), "_cpp");
        assert_eq!(param_first.type_ident.idents.len(), 1);
        assert_eq!(param_first.type_ident.idents[0].to_string(), "Pin");
        assert_eq!(param_first.type_ident.is_ref, false);
        if let QtTypes::Pin {
            ident_str,
            is_mut,
            is_this,
            type_idents,
        } = &param_first.type_ident.qt_type
        {
            assert_eq!(ident_str, "CppObj");
            assert_eq!(is_mut, &true);
            assert_eq!(is_this, &true);
            assert_eq!(type_idents.len(), 1);
            assert_eq!(type_idents[0].to_string(), "CppObj");
        } else {
            panic!();
        }

        let param_second = &invokable.parameters[1];
        assert_eq!(param_second.ident.to_string(), "string");
        // TODO: add extra checks when we read if this is a mut or not
        assert_eq!(param_second.type_ident.idents.len(), 1);
        assert_eq!(param_second.type_ident.idents[0].to_string(), "str");
        assert_eq!(param_second.type_ident.is_ref, true);

        let param_third = &invokable.parameters[2];
        assert_eq!(param_third.ident.to_string(), "number");
        assert_eq!(param_third.type_ident.idents.len(), 1);
        assert_eq!(param_third.type_ident.idents[0].to_string(), "i32");
        assert_eq!(param_third.type_ident.is_ref, false);

        // Check invokable ident
        let invokable_second = &qobject.invokables[1];
        assert_eq!(invokable_second.ident.cpp_ident.to_string(), "sayBye");
        assert_eq!(invokable_second.ident.rust_ident.to_string(), "say_bye");

        // Check invokable parameters ident and type ident
        assert_eq!(invokable_second.parameters.len(), 1);

        let param_first = &invokable_second.parameters[0];
        assert_eq!(param_first.ident.to_string(), "_cpp");
        assert_eq!(param_first.type_ident.idents.len(), 1);
        assert_eq!(param_first.type_ident.idents[0].to_string(), "Pin");
        assert_eq!(param_first.type_ident.is_ref, false);
        if let QtTypes::Pin {
            ident_str,
            is_mut,
            is_this,
            type_idents,
        } = &param_first.type_ident.qt_type
        {
            assert_eq!(ident_str, "CppObj");
            assert_eq!(is_mut, &true);
            assert_eq!(is_this, &true);
            assert_eq!(type_idents.len(), 1);
            assert_eq!(type_idents[0].to_string(), "CppObj");
        } else {
            panic!();
        }
    }
}
