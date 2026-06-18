//! Rendering Charon LLBC types into the Lean surface used by Aeneas.

#![allow(dead_code)]

use charon_lib::ast::{
    BuiltinTy, ConstantExpr, ConstantExprKind, GenericArgs, ItemId, Literal, LiteralTy, Name,
    PathElem, RefKind, ScalarValue, TranslatedCrate, Ty, TyKind, TypeDeclId, TypeDeclRef, TypeId,
};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("missing Charon name for type declaration {index}")]
    MissingTypeName { index: usize },
    #[error("unsupported LLBC type in Lean renderer: {kind}")]
    UnsupportedType { kind: &'static str },
    #[error("unsupported LLBC constant expression in Lean renderer")]
    UnsupportedConstant,
}

pub fn render_type(krate: &TranslatedCrate, ty: &Ty) -> Result<String, Error> {
    match ty.0.inner() {
        TyKind::Adt(type_ref) => render_type_ref(krate, type_ref),
        TyKind::Literal(literal) => Ok(render_literal_type(*literal).to_string()),
        TyKind::Never => Ok("Never".to_string()),
        TyKind::Ref(_, inner, _) => render_type(krate, inner),
        TyKind::RawPtr(inner, RefKind::Mut) => {
            Ok(format!("(MutRawPtr {})", render_type(krate, inner)?))
        }
        TyKind::RawPtr(inner, RefKind::Shared) => {
            Ok(format!("(ConstRawPtr {})", render_type(krate, inner)?))
        }
        TyKind::Array(inner, len) => {
            Ok(format!("(Array {} {})", render_type(krate, inner)?, render_const_expr(len)?))
        }
        TyKind::Slice(inner) => Ok(format!("(Slice {})", render_type(krate, inner)?)),
        TyKind::Pattern(inner, _) => render_type(krate, inner),
        TyKind::TypeVar(_) => Err(Error::UnsupportedType { kind: "type variable" }),
        TyKind::TraitType(_, _, _) => Err(Error::UnsupportedType { kind: "trait associated type" }),
        TyKind::DynTrait(_) => Err(Error::UnsupportedType { kind: "dyn trait" }),
        TyKind::FnPtr(_) => Err(Error::UnsupportedType { kind: "function pointer" }),
        TyKind::FnDef(_) => Err(Error::UnsupportedType { kind: "function item" }),
        TyKind::PtrMetadata(_) => Err(Error::UnsupportedType { kind: "pointer metadata" }),
        TyKind::Error(_) => Err(Error::UnsupportedType { kind: "error type" }),
    }
}

fn render_type_ref(krate: &TranslatedCrate, type_ref: &TypeDeclRef) -> Result<String, Error> {
    match &type_ref.id {
        TypeId::Adt(type_id) => render_named_type(krate, *type_id, &type_ref.generics),
        TypeId::Tuple => render_tuple(krate, &type_ref.generics),
        TypeId::Builtin(BuiltinTy::Box) => render_type_app(krate, "Box", &type_ref.generics),
        TypeId::Builtin(BuiltinTy::Str) => Ok("Str".to_string()),
    }
}

fn render_named_type(
    krate: &TranslatedCrate,
    type_id: TypeDeclId,
    generics: &GenericArgs,
) -> Result<String, Error> {
    let item_id = ItemId::Type(type_id);
    let name = krate
        .short_names
        .get(&item_id)
        .or_else(|| krate.item_names.get(&item_id))
        .ok_or_else(|| Error::MissingTypeName { index: type_id.index() })?;
    let type_name = last_ident_segment(name).ok_or(Error::UnsupportedType { kind: "ADT name" })?;
    render_type_app(krate, &type_name, generics)
}

fn render_type_app(
    krate: &TranslatedCrate,
    name: &str,
    generics: &GenericArgs,
) -> Result<String, Error> {
    let args = render_type_args(krate, generics)?;
    if args.is_empty() { Ok(name.to_string()) } else { Ok(format!("({name} {})", args.join(" "))) }
}

fn render_tuple(krate: &TranslatedCrate, generics: &GenericArgs) -> Result<String, Error> {
    let elems = render_type_args(krate, generics)?;
    match elems.as_slice() {
        [] => Ok("Unit".to_string()),
        [elem] => Ok(format!("({elem})")),
        elems => Ok(format!("({})", elems.join(" × "))),
    }
}

fn render_type_args(krate: &TranslatedCrate, generics: &GenericArgs) -> Result<Vec<String>, Error> {
    generics.types.iter().map(|ty| render_type(krate, ty)).collect()
}

fn render_literal_type(literal: LiteralTy) -> &'static str {
    match literal {
        LiteralTy::Int(int) => match int {
            charon_lib::ast::IntTy::Isize => "Std.Isize",
            charon_lib::ast::IntTy::I8 => "Std.I8",
            charon_lib::ast::IntTy::I16 => "Std.I16",
            charon_lib::ast::IntTy::I32 => "Std.I32",
            charon_lib::ast::IntTy::I64 => "Std.I64",
            charon_lib::ast::IntTy::I128 => "Std.I128",
        },
        LiteralTy::UInt(uint) => match uint {
            charon_lib::ast::UIntTy::Usize => "Std.Usize",
            charon_lib::ast::UIntTy::U8 => "Std.U8",
            charon_lib::ast::UIntTy::U16 => "Std.U16",
            charon_lib::ast::UIntTy::U32 => "Std.U32",
            charon_lib::ast::UIntTy::U64 => "Std.U64",
            charon_lib::ast::UIntTy::U128 => "Std.U128",
        },
        LiteralTy::Bool => "Bool",
        LiteralTy::Char => "Char",
        LiteralTy::Float(_) => "Float",
    }
}

fn render_const_expr(expr: &ConstantExpr) -> Result<String, Error> {
    match &expr.kind {
        ConstantExprKind::Literal(Literal::Scalar(ScalarValue::Unsigned(_, value))) => {
            Ok(value.to_string())
        }
        ConstantExprKind::Literal(Literal::Scalar(ScalarValue::Signed(_, value))) => {
            Ok(value.to_string())
        }
        _ => Err(Error::UnsupportedConstant),
    }
}

fn last_ident_segment(name: &Name) -> Option<String> {
    name.name.iter().rev().find_map(|elem| {
        if let PathElem::Ident(ident, disambiguator) = elem {
            let ident = crate::names::lean_identifier(ident);
            if disambiguator.index() == 0 {
                Some(ident)
            } else {
                Some(format!("{ident}_{}", disambiguator.index()))
            }
        } else {
            None
        }
    })
}

#[cfg(test)]
mod tests {
    use charon_lib::ast::{
        ConstantExpr, ConstantExprKind, GenericArgs, HashConsed, ItemId, Literal, LiteralTy, Name,
        PathElem, RefKind, Region, ScalarValue, TranslatedCrate, Ty, TyKind, TypeDeclId,
        TypeDeclRef, TypeId, UIntTy,
    };

    use super::*;

    fn ty(kind: TyKind) -> Ty {
        Ty(HashConsed::new(kind))
    }

    fn uint(kind: UIntTy) -> Ty {
        ty(TyKind::Literal(LiteralTy::UInt(kind)))
    }

    fn bool_ty() -> Ty {
        ty(TyKind::Literal(LiteralTy::Bool))
    }

    fn args(types: Vec<Ty>) -> Box<GenericArgs> {
        Box::new(GenericArgs {
            regions: Default::default(),
            types: types.into(),
            const_generics: Default::default(),
            trait_refs: Default::default(),
        })
    }

    fn tuple(types: Vec<Ty>) -> Ty {
        ty(TyKind::Adt(TypeDeclRef { id: TypeId::Tuple, generics: args(types) }))
    }

    #[test]
    fn renders_scalar_and_bool_types() {
        let krate = TranslatedCrate::default();

        assert_eq!(render_type(&krate, &uint(UIntTy::U32)).unwrap(), "Std.U32");
        assert_eq!(render_type(&krate, &bool_ty()).unwrap(), "Bool");
    }

    #[test]
    fn erases_references_and_renders_raw_pointers() {
        let krate = TranslatedCrate::default();
        let shared_ref = ty(TyKind::Ref(Region::Erased, uint(UIntTy::U8), RefKind::Shared));
        let mut_ptr = ty(TyKind::RawPtr(uint(UIntTy::U8), RefKind::Mut));

        assert_eq!(render_type(&krate, &shared_ref).unwrap(), "Std.U8");
        assert_eq!(render_type(&krate, &mut_ptr).unwrap(), "(MutRawPtr Std.U8)");
    }

    #[test]
    fn renders_unit_and_product_tuples() {
        let krate = TranslatedCrate::default();

        assert_eq!(render_type(&krate, &tuple(vec![])).unwrap(), "Unit");
        assert_eq!(
            render_type(&krate, &tuple(vec![uint(UIntTy::U32), bool_ty()])).unwrap(),
            "(Std.U32 × Bool)"
        );
    }

    #[test]
    fn renders_arrays_with_literal_lengths() {
        let krate = TranslatedCrate::default();
        let len = ConstantExpr {
            kind: ConstantExprKind::Literal(Literal::Scalar(ScalarValue::Unsigned(
                UIntTy::Usize,
                4,
            ))),
            ty: uint(UIntTy::Usize),
        };
        let array = ty(TyKind::Array(uint(UIntTy::U8), Box::new(len)));

        assert_eq!(render_type(&krate, &array).unwrap(), "(Array Std.U8 4)");
    }

    #[test]
    fn renders_named_adts_from_charon_item_names() {
        let type_id = TypeDeclId::ZERO;
        let mut krate = TranslatedCrate::default();
        krate.item_names.insert(
            ItemId::Type(type_id),
            Name {
                name: vec![
                    PathElem::Ident("crate".to_string(), Default::default()),
                    PathElem::Ident("Positive".to_string(), Default::default()),
                ],
            },
        );
        let adt = ty(TyKind::Adt(TypeDeclRef { id: TypeId::Adt(type_id), generics: args(vec![]) }));

        assert_eq!(render_type(&krate, &adt).unwrap(), "Positive");
    }
}
