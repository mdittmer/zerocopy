//! Lean naming helpers for Charon and Aeneas items.
//!
//! The v2 generator uses Charon item identity and names as the semantic source
//! of truth. These helpers centralize the small amount of escaping and
//! crate-relative path splitting needed to emit Lean that refers to Aeneas
//! declarations.

#![allow(dead_code)]

use charon_lib::ast::{
    GenericArgs, GenericParams, ImplElem, ItemId, Name, PathElem, TranslatedCrate, Ty, TyKind,
    TypeId,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeanItemPath {
    pub crate_namespace: String,
    pub relative_namespace: Vec<String>,
    pub item: String,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Charon item name is empty")]
    EmptyPath,
    #[error("trait impl item paths are not supported yet")]
    UnsupportedTraitImplPath,
    #[error("target-specific item paths are not supported yet")]
    UnsupportedTargetPath,
    #[error("inherent impl self type is not a named ADT")]
    UnsupportedImplSelfType,
    #[error("missing Charon name for type declaration {index}")]
    MissingTypeName { index: usize },
    #[error("missing Charon trait declaration {index}")]
    MissingTraitName { index: usize },
    #[error("missing Charon trait implementation {index}")]
    MissingTraitImpl { index: usize },
    #[error(transparent)]
    Type(#[from] crate::lean_types::Error),
}

pub fn crate_namespace(krate: &TranslatedCrate) -> String {
    lean_identifier(&krate.crate_name)
}

pub fn item_path(krate: &TranslatedCrate, name: &Name) -> Result<LeanItemPath, Error> {
    let crate_namespace = crate_namespace(krate);
    let mut parts = Vec::new();

    for elem in &name.name {
        match elem {
            PathElem::Ident(ident, disambiguator) => {
                parts.push(lean_identifier_with_disambiguator(ident, disambiguator.index()));
            }
            PathElem::Impl(ImplElem::Ty(ty)) => {
                parts.push(inherent_impl_owner(krate, &ty.skip_binder)?);
            }
            PathElem::Impl(ImplElem::Trait(impl_id)) => {
                parts.extend(trait_impl_namespace(krate, *impl_id)?);
            }
            PathElem::Instantiated(_) => {}
            PathElem::Target(_) => return Err(Error::UnsupportedTargetPath),
        }
    }

    if parts.first() == Some(&crate_namespace) {
        parts.remove(0);
    }
    let item = parts.pop().ok_or(Error::EmptyPath)?;

    Ok(LeanItemPath { crate_namespace, relative_namespace: parts, item })
}

pub fn lean_identifier(raw: &str) -> String {
    let raw = raw.replace('-', "_");
    let mut ident = String::with_capacity(raw.len());
    for (index, ch) in raw.chars().enumerate() {
        let valid = if index == 0 {
            ch == '_' || ch.is_ascii_alphanumeric()
        } else {
            ch == '_' || ch == '\'' || ch.is_ascii_alphanumeric()
        };
        ident.push(if valid { ch } else { '_' });
    }
    if ident.is_empty() {
        ident.push('_');
    }
    if ident.as_bytes()[0].is_ascii_digit() {
        ident.insert(0, '_');
    }
    if is_lean_keyword(&ident) {
        ident.push('1');
    }
    ident
}

impl LeanItemPath {
    pub fn spec_namespace_segments(&self) -> Vec<String> {
        self.relative_namespace.iter().chain(std::iter::once(&self.item)).cloned().collect()
    }

    pub fn aeneas_name(&self) -> String {
        std::iter::once(&self.crate_namespace)
            .chain(&self.relative_namespace)
            .chain(std::iter::once(&self.item))
            .map(String::as_str)
            .collect::<Vec<_>>()
            .join(".")
    }
}

fn lean_identifier_with_disambiguator(raw: &str, disambiguator: usize) -> String {
    let ident = lean_identifier(raw);
    if disambiguator == 0 { ident } else { format!("{ident}_{disambiguator}") }
}

fn inherent_impl_owner(krate: &TranslatedCrate, ty: &Ty) -> Result<String, Error> {
    let TyKind::Adt(type_ref) = ty.0.inner() else {
        return Err(Error::UnsupportedImplSelfType);
    };
    let TypeId::Adt(type_id) = &type_ref.id else {
        return Err(Error::UnsupportedImplSelfType);
    };
    let item_id = ItemId::Type(*type_id);
    let name = krate
        .short_names
        .get(&item_id)
        .or_else(|| krate.item_names.get(&item_id))
        .ok_or_else(|| Error::MissingTypeName { index: type_id.index() })?;
    last_ident_segment(name).ok_or(Error::UnsupportedImplSelfType)
}

fn trait_impl_namespace(
    krate: &TranslatedCrate,
    impl_id: charon_lib::ast::TraitImplId,
) -> Result<Vec<String>, Error> {
    let trait_impl =
        krate.trait_impls.get(impl_id).ok_or(Error::MissingTraitImpl { index: impl_id.index() })?;
    let self_ty = trait_impl
        .impl_trait
        .generics
        .types
        .iter()
        .next()
        .ok_or(Error::UnsupportedTraitImplPath)?;
    let self_name = adt_self_type_owner(krate, self_ty)?;

    let trait_decl = krate
        .trait_decls
        .get(trait_impl.impl_trait.id)
        .ok_or(Error::MissingTraitName { index: trait_impl.impl_trait.id.index() })?;
    let trait_path = item_path(krate, &trait_decl.item_meta.name)?;
    if trait_path.crate_namespace != crate_namespace(krate)
        || !trait_path.relative_namespace.is_empty()
    {
        return Err(Error::UnsupportedTraitImplPath);
    }

    Ok(vec![
        self_name,
        "Insts".to_string(),
        trait_impl_segment_for_ref(
            krate,
            &trait_impl.generics,
            &trait_path.crate_namespace,
            &trait_path.item,
            &trait_impl.impl_trait.generics,
        )?,
    ])
}

fn adt_self_type_owner(krate: &TranslatedCrate, ty: &Ty) -> Result<String, Error> {
    let TyKind::Adt(type_ref) = ty.0.inner() else {
        return Err(Error::UnsupportedTraitImplPath);
    };
    if !type_ref.generics.trait_refs.is_empty() {
        return Err(Error::UnsupportedTraitImplPath);
    }
    let TypeId::Adt(type_id) = &type_ref.id else {
        return Err(Error::UnsupportedTraitImplPath);
    };
    let item_id = ItemId::Type(*type_id);
    let name = krate
        .short_names
        .get(&item_id)
        .or_else(|| krate.item_names.get(&item_id))
        .ok_or_else(|| Error::MissingTypeName { index: type_id.index() })?;
    last_ident_segment(name).ok_or(Error::UnsupportedTraitImplPath)
}

fn trait_impl_segment(crate_namespace: &str, trait_name: &str) -> String {
    format!("{}{}", uppercase_first(crate_namespace), trait_name)
}

fn trait_impl_segment_for_ref(
    krate: &TranslatedCrate,
    impl_generics: &GenericParams,
    crate_namespace: &str,
    trait_name: &str,
    trait_args: &GenericArgs,
) -> Result<String, Error> {
    let mut segment = trait_impl_segment(crate_namespace, trait_name);
    let rendered_args =
        crate::lean_types::render_generic_args_with_generics(krate, impl_generics, trait_args)?;
    for arg in rendered_args.into_iter().skip(1) {
        segment.push_str(&trait_impl_arg_segment(&arg));
    }
    Ok(segment)
}

fn trait_impl_arg_segment(arg: &str) -> String {
    arg.split(|ch: char| !(ch == '_' || ch == '\'' || ch == '.' || ch.is_ascii_alphanumeric()))
        .filter(|part| !part.is_empty())
        .filter_map(|part| part.rsplit('.').next())
        .map(lean_identifier)
        .collect::<Vec<_>>()
        .join("")
}

fn uppercase_first(value: &str) -> String {
    let mut chars = value.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

fn last_ident_segment(name: &Name) -> Option<String> {
    name.name.iter().rev().find_map(|elem| {
        if let PathElem::Ident(ident, disambiguator) = elem {
            Some(lean_identifier_with_disambiguator(ident, disambiguator.index()))
        } else {
            None
        }
    })
}

fn is_lean_keyword(ident: &str) -> bool {
    matches!(
        ident,
        "abbrev"
            | "axiom"
            | "by"
            | "calc"
            | "class"
            | "def"
            | "deriving"
            | "do"
            | "else"
            | "end"
            | "example"
            | "from"
            | "fun"
            | "have"
            | "if"
            | "import"
            | "in"
            | "inductive"
            | "instance"
            | "let"
            | "match"
            | "namespace"
            | "open"
            | "opaque"
            | "rec"
            | "section"
            | "structure"
            | "then"
            | "theorem"
            | "universe"
            | "variable"
            | "where"
            | "with"
    )
}

#[cfg(test)]
mod tests {
    use charon_lib::ast::{
        DeBruijnVar, Disambiguator, GenericArgs, HashConsed, ItemId, Name, PathElem,
        TranslatedCrate, Ty, TyKind, TypeDeclId, TypeDeclRef, TypeId, TypeVarId,
    };

    use super::*;

    fn ident(raw: &str) -> PathElem {
        PathElem::Ident(raw.to_string(), Disambiguator::ZERO)
    }

    fn ty(kind: TyKind) -> Ty {
        Ty(HashConsed::new(kind))
    }

    fn args(types: Vec<Ty>) -> Box<GenericArgs> {
        Box::new(GenericArgs {
            regions: Default::default(),
            types: types.into(),
            const_generics: Default::default(),
            trait_refs: Default::default(),
        })
    }

    fn type_var() -> Ty {
        ty(TyKind::TypeVar(DeBruijnVar::Free(TypeVarId::ZERO)))
    }

    #[test]
    fn lean_identifier_normalizes_common_rust_names() {
        assert_eq!(lean_identifier("my-crate"), "my_crate");
        assert_eq!(lean_identifier("theorem"), "theorem1");
        assert_eq!(lean_identifier("3d"), "_3d");
        assert_eq!(lean_identifier("has space"), "has_space");
    }

    #[test]
    fn item_path_splits_crate_relative_namespace() {
        let krate = TranslatedCrate { crate_name: "test-impl".into(), ..Default::default() };
        let name = Name { name: vec![ident("test_impl"), ident("inner"), ident("f")] };

        let path = item_path(&krate, &name).unwrap();

        assert_eq!(path.crate_namespace, "test_impl");
        assert_eq!(path.relative_namespace, ["inner"]);
        assert_eq!(path.item, "f");
        assert_eq!(path.spec_namespace_segments(), ["inner", "f"]);
        assert_eq!(path.aeneas_name(), "test_impl.inner.f");
    }

    #[test]
    fn item_path_keeps_nonzero_disambiguators() {
        let krate = TranslatedCrate { crate_name: "crate".into(), ..Default::default() };
        let name = Name {
            name: vec![
                ident("crate"),
                PathElem::Ident("helper".to_string(), Disambiguator::from_usize(2)),
                ident("f"),
            ],
        };

        let path = item_path(&krate, &name).unwrap();

        assert_eq!(path.relative_namespace, ["helper_2"]);
        assert_eq!(path.aeneas_name(), "crate.helper_2.f");
    }

    #[test]
    fn trait_impl_segment_matches_observed_aeneas_shape() {
        assert_eq!(
            trait_impl_segment("anneal_v2_generate_smoke", "ReadVal"),
            "Anneal_v2_generate_smokeReadVal"
        );
    }

    #[test]
    fn trait_impl_arg_segment_compacts_rendered_lean_types() {
        assert_eq!(trait_impl_arg_segment("Std.U32"), "U32");
        assert_eq!(trait_impl_arg_segment("(Wrapper T)"), "WrapperT");
        assert_eq!(trait_impl_arg_segment("(ConstWrapper T N)"), "ConstWrapperTN");
    }

    #[test]
    fn trait_impl_owner_uses_base_adt_for_generic_self_type() {
        let type_id = TypeDeclId::ZERO;
        let mut krate = TranslatedCrate::default();
        krate
            .item_names
            .insert(ItemId::Type(type_id), Name { name: vec![ident("crate"), ident("Wrapper")] });
        let wrapper_of_t = ty(TyKind::Adt(TypeDeclRef {
            id: TypeId::Adt(type_id),
            generics: args(vec![type_var()]),
        }));

        assert_eq!(adt_self_type_owner(&krate, &wrapper_of_t).unwrap(), "Wrapper");
    }
}
