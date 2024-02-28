use proc_macro2::{Span, TokenStream};
use syn::Ident;

use crate::pathtree::PathTree;

#[derive(Debug, Clone, Copy)]
pub enum IntType {
    I8,
    U8,
    I16,
    U16,
    I32,
    U32,
    I64,
    U64,
    Isize,
    Usize,
}

impl IntType {
    pub(crate) fn type_name(self) -> Ident {
        let t = match self {
            IntType::I8 => "i8",
            IntType::U8 => "u8",
            IntType::I16 => "i16",
            IntType::U16 => "u16",
            IntType::I32 => "i32",
            IntType::U32 => "u32",
            IntType::I64 => "i64",
            IntType::U64 => "u64",
            IntType::Isize => "isize",
            IntType::Usize => "usize",
        };
        Ident::new(t, Span::call_site())
    }

    pub(crate) fn is_signed(self) -> bool {
        matches!(
            self,
            IntType::I8 | IntType::I16 | IntType::I32 | IntType::Isize | IntType::I64
        )
    }
}

#[derive(Debug, Clone)]
pub enum CustomField {
    Type(String),
    Delegate(String),
}

macro_rules! config_decl {
    ($($(#[$attr:meta])* $([$placeholder:ident])? $field:ident : $([$placeholder2:ident])? Option<$type:ty>,)+) => {
        #[non_exhaustive]
        #[derive(Debug, Clone, Default)]
        pub struct Config {
            $($(#[$attr])* pub $field: Option<$type>,)+
        }

        impl Config {
            pub fn new() -> Self {
                Self::default()
            }

            pub fn merge(&mut self, other: &Self) {
                $(config_decl!(@merge $([$placeholder])? $field, self, other);)+
            }

            $(config_decl!(@setter $field: $([$placeholder2])? $type);)+
        }
    };

    (@merge $field:ident, $self:ident, $other:ident) => {
        if let Some(v) = &$other.$field {
            $self.$field = Some(v.clone());
        }
    };

    (@merge [no_inherit] $field:ident, $self:ident, $other:ident) => {
        $self.$field = $other.$field.clone();
    };

    (@setter $field:ident: [deref] $type:ty) => {
        pub fn $field(mut self, s: &str) -> Self {
            self.$field = Some(s.to_owned());
            self
        }
    };

    (@setter $field:ident: $type:ty) => {
        pub fn $field(mut self, val: $type) -> Self {
            self.$field = Some(val);
            self
        }
    };
}

config_decl! {
    // Field configs
    max_len: Option<u32>,
    max_bytes: Option<u32>,
    int_type: Option<IntType>,
    field_attributes: [deref] Option<String>,
    boxed: Option<bool>,
    vec_type: [deref] Option<String>,
    string_type: [deref] Option<String>,
    map_type: [deref] Option<String>,
    no_hazzer: Option<bool>,
    [no_inherit] custom_field: Option<CustomField>,
    [no_inherit] rename_field: [deref] Option<String>,

    // Type configs
    enum_int_type: Option<IntType>,
    type_attributes: [deref] Option<String>,
    hazzer_attributes: [deref] Option<String>,
    no_debug_derive: Option<bool>,

    // General configs
    skip: Option<bool>,
}

impl Config {
    pub(crate) fn field_attr_parsed(&self) -> TokenStream {
        // TODO handle parse error
        syn::parse_str(self.field_attributes.as_deref().unwrap_or("")).unwrap()
    }

    pub(crate) fn type_attr_parsed(&self) -> TokenStream {
        // TODO handle parse error
        syn::parse_str(self.type_attributes.as_deref().unwrap_or("")).unwrap()
    }

    pub(crate) fn hazzer_attr_parsed(&self) -> TokenStream {
        // TODO handle parse error
        syn::parse_str(self.hazzer_attributes.as_deref().unwrap_or("")).unwrap()
    }

    pub(crate) fn rust_field_name(&self, name: &str) -> Ident {
        // TODO handle parse error
        syn::parse_str(self.rename_field.as_deref().unwrap_or(name)).unwrap()
    }

    pub(crate) fn vec_type_parsed(&self) -> Option<syn::Path> {
        // TODO handle parse error
        self.vec_type.as_ref().map(|t| syn::parse_str(t).unwrap())
    }

    pub(crate) fn string_type_parsed(&self) -> Option<syn::Path> {
        // TODO handle parse error
        self.string_type
            .as_ref()
            .map(|t| syn::parse_str(t).unwrap())
    }

    pub(crate) fn map_type_parsed(&self) -> Option<syn::Path> {
        // TODO handle parse error
        self.map_type.as_ref().map(|t| syn::parse_str(t).unwrap())
    }

    pub(crate) fn custom_field_parsed(&self) -> Option<crate::generator::CustomField> {
        // TODO handle parse error
        match &self.custom_field {
            Some(CustomField::Type(s)) => Some(crate::generator::CustomField::Type(
                syn::parse_str(s).unwrap(),
            )),
            Some(CustomField::Delegate(s)) => Some(crate::generator::CustomField::Delegate(
                syn::parse_str(s).unwrap(),
            )),
            None => todo!(),
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
enum EncodeDecode {
    EncodeOnly,
    DecodeOnly,
    #[default]
    Both,
}

pub struct GenConfig {
    pub(crate) encode_decode: EncodeDecode,
    pub(crate) size_cache: bool,
    pub(crate) default_pkg_filename: String,
    pub(crate) strip_enum_prefix: bool,
    pub(crate) format: bool,
    pub(crate) use_std: bool,

    pub(crate) field_configs: PathTree<Box<Config>>,
}

#[cfg(test)]
mod tests {
    use quote::{format_ident, quote, ToTokens};

    use super::*;

    #[test]
    fn merge() {
        let mut mergee = Config::new()
            .rename_field("rename")
            .skip(true)
            .vec_type("vec")
            .string_type("str");
        let merger = Config::new().skip(false).vec_type("array");
        mergee.merge(&merger);

        assert!(!mergee.skip.unwrap());
        assert_eq!(mergee.vec_type.unwrap(), "array");
        assert_eq!(mergee.string_type.unwrap(), "str");
        // max_len was never set
        assert!(mergee.max_len.is_none());
        // rename_field gets overwritten unconditionally when merging
        assert!(mergee.rename_field.is_none());
    }

    #[test]
    fn parse() {
        let mut config = Config::new()
            .vec_type("heapless::Vec")
            .string_type("heapless::String")
            .map_type("Map")
            .hazzer_attributes("#[derive(Eq)]")
            .type_attributes("#[derive(Hash)]");

        assert_eq!(
            config
                .vec_type_parsed()
                .unwrap()
                .to_token_stream()
                .to_string(),
            quote! { heapless::Vec }.to_string()
        );
        assert_eq!(
            config
                .string_type_parsed()
                .unwrap()
                .to_token_stream()
                .to_string(),
            quote! { heapless::String }.to_string()
        );
        assert_eq!(
            config
                .map_type_parsed()
                .unwrap()
                .to_token_stream()
                .to_string(),
            "Map"
        );
        assert_eq!(
            config.hazzer_attr_parsed().to_string(),
            quote! { #[derive(Eq)] }.to_string()
        );
        assert_eq!(
            config.type_attr_parsed().to_string(),
            quote! { #[derive(Hash)] }.to_string()
        );

        assert_eq!(config.field_attr_parsed().to_string(), "");
        config.field_attributes = Some("#[default]".to_owned());
        assert_eq!(
            config.field_attr_parsed().to_string(),
            quote! { #[default] }.to_string()
        );

        assert_eq!(config.rust_field_name("name"), format_ident!("name"));
        config.rename_field = Some("rename".to_string());
        assert_eq!(config.rust_field_name("name"), format_ident!("rename"));

        config.custom_field = Some(CustomField::Type("Vec<u16, 4>".to_owned()));
        let crate::generator::CustomField::Type(typ) = config.custom_field_parsed().unwrap() else {
            unreachable!()
        };
        assert_eq!(
            typ.to_token_stream().to_string(),
            quote! { Vec<u16, 4> }.to_string()
        );

        config.custom_field = Some(CustomField::Delegate("name".to_owned()));
        let crate::generator::CustomField::Delegate(del) = config.custom_field_parsed().unwrap()
        else {
            unreachable!()
        };
        assert_eq!(del, format_ident!("name"));
    }
}
