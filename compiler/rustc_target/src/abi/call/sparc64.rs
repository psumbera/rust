// FIXME: This needs an audit for correctness and completeness.

use crate::abi::call::{ArgAbi, ArgAttribute, CastTarget, FnAbi, PassMode, Reg, RegKind, Uniform};
use crate::abi::{self, HasDataLayout, LayoutOf, Size, TyAndLayout, TyAndLayoutMethods};

fn is_homogeneous_aggregate<'a, Ty, C>(cx: &C, arg: &mut ArgAbi<'a, Ty>) -> Option<Uniform>
where
    Ty: TyAndLayoutMethods<'a, C> + Copy,
    C: LayoutOf<Ty = Ty, TyAndLayout = TyAndLayout<'a, Ty>> + HasDataLayout,
{
    arg.layout.homogeneous_aggregate(cx).ok().and_then(|ha| ha.unit()).and_then(|unit| {
        // Ensure we have at most eight uniquely addressable members.
        if arg.layout.size > unit.size.checked_mul(8, cx).unwrap() {
            return None;
        }

        if unit.kind == RegKind::Float {
            return None;
        }

        let valid_unit = match unit.kind {
            RegKind::Integer => false,
            RegKind::Float => true,
            RegKind::Vector => arg.layout.size.bits() == 128,
        };

        valid_unit.then_some(Uniform { unit, total: arg.layout.size })
    })
}

fn classify_ret<'a, Ty, C>(cx: &C, ret: &mut ArgAbi<'a, Ty>)
where
    Ty: TyAndLayoutMethods<'a, C> + Copy,
    C: LayoutOf<Ty = Ty, TyAndLayout = TyAndLayout<'a, Ty>> + HasDataLayout,
{
    if !ret.layout.is_aggregate() {
        ret.extend_integer_width_to(64);
        return;
    }

    // should we treat floats specially too?
    if let Some(uniform) = is_homogeneous_aggregate(cx, ret) {
        ret.cast_to(uniform);
        return;
    }

    let mut has_8_or_16_bits = false;

    let mut prefix = [None; 8];
    let mut prefix_index = 0;

    match ret.layout.fields {
        abi::FieldsShape::Primitive |
        abi::FieldsShape::Array { .. } |
        abi::FieldsShape::Union(_) => { }

        // Handle structures with floats (can we have here structure without float, guess not)
        abi::FieldsShape::Arbitrary { .. } => {

            let mut has_float = false;
            let mut size = 0;

            for i in 0..ret.layout.fields.count() {

                let field = ret.layout.field(cx, i);

                if let abi::Abi::Scalar(scalar) = &field.abi {
                    if let abi::F32 = scalar.value {
                        size += 4;
                        has_float = true;
                        prefix[prefix_index] = Some(RegKind::Float);
                        prefix_index += 1;
                    } else if let abi::F64 = scalar.value {
                        size += 8;
                        has_float = true;
                        prefix[prefix_index] = Some(RegKind::Float);
                        prefix_index += 1;
                    } else if let abi::Int(i, _signed) = scalar.value {
                        //size += i.size().bits()/8;
                        // one register is used to pass more i8/i16 types
                        if has_8_or_16_bits == false {
                            prefix[prefix_index] = Some(RegKind::Integer);
                            prefix_index += 1;
                        }
                        if i.size().bits() == 8 || i.size().bits() == 16 {
                          has_8_or_16_bits = true;
                        }
                    } else if let abi::Pointer = scalar.value {
                        // should care about 8 or 16 bits too?
                        size += 8;
                    }
                }
            }

            // bigger structures will be passed indirecrtly
            if has_float && size <= 16 {

                // special case needs i8 and i16 types
                if has_8_or_16_bits {
                     ret.cast_to(CastTarget {
                                prefix,
                                prefix_chunk_size: Size::from_bytes(8),
                                rest: Uniform { unit: Reg::i64(), total: Size::from_bytes(0) },
                     });
                     return;
                 }

                 // to pass in registers InReg flag need to be set
                 if let PassMode::Direct(ref mut attrs) = ret.mode {
                     attrs.set(ArgAttribute::InReg);
                     return;
                 } else if let PassMode::Pair(ref mut attrs1, ref mut attrs2) = ret.mode {
                     attrs1.set(ArgAttribute::InReg);
                     attrs2.set(ArgAttribute::InReg);
                     return;
                 }
             }
        }
    };

    let size = ret.layout.size;
    let bits = size.bits();
    if bits <= 256 {
        let unit = Reg::i64();
        ret.cast_to(Uniform { unit, total: size });
        return;
    }

    // don't return aggregates in registers
    ret.make_indirect();
}

fn classify_arg<'a, Ty, C>(cx: &C, arg: &mut ArgAbi<'a, Ty>)
where
    Ty: TyAndLayoutMethods<'a, C> + Copy,
    C: LayoutOf<Ty = Ty, TyAndLayout = TyAndLayout<'a, Ty>> + HasDataLayout,
{
    if !arg.layout.is_aggregate() {
        arg.extend_integer_width_to(64);
        return;
    }

    // This doesn't intentionally handle structures with floats which needs
    // special care below.
    if let Some(uniform) = is_homogeneous_aggregate(cx, arg) {
        arg.cast_to(uniform);
        return;
    }

    let mut has_8_or_16_bits = false;

    let mut prefix = [None; 8];
    let mut prefix_index = 0;

    match arg.layout.fields {
        abi::FieldsShape::Primitive |
        abi::FieldsShape::Array { .. } |
        abi::FieldsShape::Union(_) => { }

        // Handle structures with floats (can we have here structure without float, guess not)
        abi::FieldsShape::Arbitrary { .. } => {

            let mut has_float = false;
            let mut size = 0;

            for i in 0..arg.layout.fields.count() {

                let field = arg.layout.field(cx, i);

                if let abi::Abi::Scalar(scalar) = &field.abi {
                    if let abi::F32 = scalar.value {
                        size += 4;
                        has_float = true;
                        prefix[prefix_index] = Some(RegKind::Float);
                        prefix_index += 1;
                    } else if let abi::F64 = scalar.value {
                        size += 8;
                        has_float = true;
                        prefix[prefix_index] = Some(RegKind::Float);
                        prefix_index += 1;
                    } else if let abi::Int(i, _signed) = scalar.value {
                        size += i.size().bits()/8;
                        // one register is used to pass more i8/i16 types
                        if has_8_or_16_bits == false {
                            prefix[prefix_index] = Some(RegKind::Integer);
                            prefix_index += 1;
                        }
                        if i.size().bits() == 8 || i.size().bits() == 16 {
                          has_8_or_16_bits = true;
                        }
                    } else if let abi::Pointer = scalar.value {
                        // should care about 8 or 16 bits too?
                        size += 8;
                    }
                }
            }

            // bigger structures will be passed indirecrtly
            if has_float && size <= 16 {

                // special case needs i8 and i16 types
                if has_8_or_16_bits {
                     arg.cast_to(CastTarget {
                                prefix,
                                prefix_chunk_size: Size::from_bytes(8),
                                rest: Uniform { unit: Reg::i64(), total: Size::from_bytes(0) },
                     });
                     return;
                 }

                 // to pass in registers InReg flag need to be set
                 if let PassMode::Direct(ref mut attrs) = arg.mode {
                     attrs.set(ArgAttribute::InReg);
                     return;
                 } else if let PassMode::Pair(ref mut attrs1, ref mut attrs2) = arg.mode {
                     attrs1.set(ArgAttribute::InReg);
                     attrs2.set(ArgAttribute::InReg);
                     return;
                 }
             }
        }
    };

    let total = arg.layout.size;
    if total.bits() > 128 {
        arg.make_indirect();
        return;
    }

    arg.cast_to(Uniform { unit: Reg::i64(), total });
}

pub fn compute_abi_info<'a, Ty, C>(cx: &C, fn_abi: &mut FnAbi<'a, Ty>)
where
    Ty: TyAndLayoutMethods<'a, C> + Copy,
    C: LayoutOf<Ty = Ty, TyAndLayout = TyAndLayout<'a, Ty>> + HasDataLayout,
{
    if !fn_abi.ret.is_ignore() {
        classify_ret(cx, &mut fn_abi.ret);
    }

    for arg in &mut fn_abi.args {
        if arg.is_ignore() {
            continue;
        }
        classify_arg(cx, arg);
    }
}
