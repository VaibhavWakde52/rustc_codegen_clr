
use crate::cil_op::{CILOp, FieldDescriptor};
use crate::r#type::Type;
use rustc_middle::mir::{Place, PlaceElem};
use rustc_middle::ty::{FloatTy, Instance, IntTy, ParamEnv, Ty, TyCtxt, TyKind, UintTy};
use crate::utilis::field_name;

pub(super) fn local_get(local: usize, method: &rustc_middle::mir::Body) -> CILOp {
    if local == 0 {
        CILOp::LDLoc(0)
    } else if local > method.arg_count {
        CILOp::LDLoc((local - method.arg_count) as u32)
    } else {
        CILOp::LDArg((local - 1) as u32)
    }
}
/// Returns the ops for getting the value of place.
pub fn place_get<'a>(
    place: &Place<'a>,
    ctx: TyCtxt<'a>,
    method: &rustc_middle::mir::Body<'a>,
    method_instance: Instance<'a>,
) -> Vec<CILOp> {
    let mut ops = Vec::with_capacity(place.projection.len());
    if place.projection.is_empty() {
        ops.push(local_get(place.local.as_usize(), method));
        ops
    } else {
        let (op, mut ty) = super::local_body(place.local.as_usize(), method);
        ty = crate::utilis::monomorphize(&method_instance, ty, ctx);
        let mut ty = ty.into();
        ops.push(op);
        let (head, body) = super::slice_head(place.projection);
        for elem in body {
            println!("elem:{elem:?} ty:{ty:?}");
            let (curr_ty, curr_ops) = super::place_elem_body(elem, ty, ctx, method_instance, method);
            ty = curr_ty.monomorphize(&method_instance, ctx);
            ops.extend(curr_ops);
        }
        ops.extend(place_elem_get(head, ty, ctx, method_instance));
        ops
    }
}
fn place_elem_get_at<'a>(
    curr_type: super::PlaceTy<'a>,
    ctx: TyCtxt<'a>,
    method_instance: &Instance<'a>,
) -> Vec<CILOp> {
    let curr_ty = curr_type.as_ty().expect("Can't index into enum!");
    let tpe = Type::from_ty(curr_ty, ctx, method_instance);
    let class = if let Type::DotnetType(dotnet) = &tpe {
        dotnet
    } else {
        panic!("Can't index into type {tpe:?}");
    };
    let index_ty = Type::USize;
    let _element_ty = crate::r#type::element_type(curr_ty);

    let signature = crate::function_sig::FnSig::new(&[tpe.clone(), index_ty], &Type::GenericArg(0));
    vec![CILOp::Call(crate::cil_op::CallSite::boxed(
        Some(class.as_ref().clone()),
        "get_Item".into(),
        signature,
        false,
    ))]
}
fn place_elem_get<'a>(
    place_elem: &PlaceElem<'a>,
    curr_type: super::PlaceTy<'a>,
    ctx: TyCtxt<'a>,
    method_instance: Instance<'a>,
) -> Vec<CILOp> {
    match place_elem {
        PlaceElem::Deref => super::deref_op(super::pointed_type(curr_type).into(), ctx, &method_instance),
        PlaceElem::Field(index, _field_type) => match curr_type {
            super::PlaceTy::Ty(curr_type) => {
                let curr_type = crate::utilis::monomorphize(&method_instance, curr_type, ctx);
                let field_type = crate::utilis::generic_field_ty(
                    curr_type,
                    index.as_u32(),
                    ctx,
                    method_instance,
                );

                let field_name = field_name(curr_type, index.as_u32());
                println!("Generic type of field named {field_name:?} is {field_type:?}");
                let curr_type = crate::r#type::Type::from_ty(curr_type, ctx, &method_instance);
                let curr_type = if let crate::r#type::Type::DotnetType(dotnet_type) = curr_type {
                    dotnet_type.as_ref().clone()
                } else {
                    panic!();
                };
                let field_desc = FieldDescriptor::boxed(curr_type, field_type, field_name);
                vec![CILOp::LDField(field_desc)]
            }
            super::PlaceTy::EnumVariant(enm, var_idx) => {
                let owner = crate::utilis::monomorphize(&method_instance, enm, ctx);
                let variant_name = crate::utilis::variant_name(owner, var_idx);
                let owner = crate::utilis::monomorphize(&method_instance, enm, ctx);
                let field_type =
                    crate::utilis::generic_field_ty(owner, index.as_u32(), ctx, method_instance);
                let owner = crate::r#type::Type::from_ty(owner, ctx, &method_instance);
                let owner = if let crate::r#type::Type::DotnetType(owner) = owner {
                    owner.as_ref().clone()
                } else {
                    panic!();
                };
                let field_name = field_name(enm, index.as_u32());
                let mut field_owner = owner;

                field_owner.append_path(&format!("/{variant_name}"));
                let field_desc = FieldDescriptor::boxed(field_owner, field_type, field_name);
                let ops = vec![CILOp::LDField(field_desc)];
                println!("Using ops:{ops:?} to get field of an enum variant!");
                ops
                //todo!("Can't get fields of enum variants yet!");
            }
        },
        PlaceElem::Index(index) => {
            let mut ops = vec![crate::place::local_adress(
                index.as_usize(),
                ctx.optimized_mir(method_instance.def_id()),
            )];
            ops.extend(place_elem_get_at(curr_type, ctx, &method_instance));
            ops
        }
        PlaceElem::ConstantIndex {
            offset,
            min_length: _,
            from_end,
        } => {
            let mut ops = if !from_end {
                vec![CILOp::LdcI64(*offset as i64)]
            } else {
                let mut get_len = super::place_get_length(curr_type, ctx, method_instance);
                get_len.extend(vec![CILOp::LdcI64(*offset as i64), CILOp::Sub]);
                get_len
            };
            ops.extend(place_elem_get_at(curr_type, ctx, &method_instance));
            ops
        }
        _ => todo!("Can't handle porojection {place_elem:?} in get"),
    }
}