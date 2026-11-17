use rust16::machine::Segment;
use rustc_abi::{TagEncoding, VariantIdx, Variants};
use rustc_middle::{
    mir::*,
    ty::{
        self, Ty, TyCtxt,
        layout::{LayoutCx, TyAndLayout},
    },
};

pub fn layout<'tcx>(tcx: TyCtxt<'tcx>, ty: Ty<'tcx>) -> Result<TyAndLayout<'tcx>, String> {
    tcx.layout_of(ty::TypingEnv::fully_monomorphized().as_query_input(ty))
        .map_err(|e| format!("layout of {ty}: {e:?}"))
}

pub struct Slots {
    pub offsets: Vec<i16>,
    pub frame: u16,
}
impl Slots {
    pub fn new<'tcx>(tcx: TyCtxt<'tcx>, body: &Body<'tcx>) -> Result<Self, String> {
        let mut offsets = vec![0; body.local_decls.len()];
        let mut args = if layout(tcx, body.return_ty())?.size.bytes() > 4 {
            8i32
        } else {
            6i32
        };
        let mut frame = 0i32;
        for (i, decl) in body.local_decls.iter_enumerated().rev() {
            let bytes = layout(tcx, decl.ty)?.size.bytes();
            let words = i32::try_from(bytes.div_ceil(2)).map_err(|_| "local too large")?;
            let at = if (1..=body.arg_count).contains(&i.as_usize()) {
                let start = args;
                args += words * 2;
                start
            } else {
                frame += words * 2;
                -frame
            };
            if frame + args > 16000 {
                return Err("frame exceeds DOS stack".into());
            }
            offsets[i.as_usize()] = at as i16;
        }
        Ok(Self {
            offsets,
            frame: frame as u16,
        })
    }
}

pub fn transfer(code: &mut Segment, at: i16, bytes: u64, store: bool) -> Result<(), String> {
    let mut part = |offset: i16, width: u64, high: bool| {
        if width == 1 && !store {
            code.emit(if high { &[0x31, 0xd2] } else { &[0x31, 0xc0] });
        }
        let opcode = (if store { 0x88 } else { 0x8a }) + u8::from(width == 2);
        code.bp_operand(opcode, if high { 2 } else { 0 }, offset);
    };
    if bytes > 4 {
        return Err(format!("{bytes}-byte register transfer unsupported"));
    }
    if bytes > 0 {
        part(at, bytes.min(2), false);
    }
    if bytes > 2 {
        part(at + 2, bytes - 2, true);
    }
    Ok(())
}

pub fn address<'tcx>(
    tcx: TyCtxt<'tcx>,
    body: &Body<'tcx>,
    slots: &Slots,
    p: Place<'tcx>,
) -> Result<(i16, TyAndLayout<'tcx>), String> {
    let mut at = slots.offsets[p.local.as_usize()];
    let mut data = layout(tcx, body.local_decls[p.local].ty)?;
    let cx = LayoutCx::new(tcx, ty::TypingEnv::fully_monomorphized());
    for projection in p.projection {
        match projection {
            ProjectionElem::Downcast(_, variant) => data = data.for_variant(&cx, variant),
            ProjectionElem::Field(field, _) => {
                at += data.fields.offset(field.as_usize()).bytes() as i16;
                data = data.field(&cx, field.as_usize());
            }
            ProjectionElem::ConstantIndex {
                offset,
                from_end: false,
                ..
            } => {
                let element = data.ty.builtin_index().ok_or("constant array index")?;
                data = layout(tcx, element)?;
                at = at
                    .checked_add(
                        i16::try_from(offset * data.size.bytes())
                            .map_err(|_| "constant index exceeds near frame")?,
                    )
                    .ok_or("constant index outside frame")?;
            }
            _ => return Err(format!("unsupported place {p:?}")),
        }
    }
    Ok((at, data))
}

pub fn tag_value<'tcx>(
    layout: TyAndLayout<'tcx>,
    tcx: TyCtxt<'tcx>,
    variant: VariantIdx,
) -> Option<(i16, u64, u128)> {
    match &layout.variants {
        Variants::Multiple {
            tag,
            tag_encoding,
            tag_field,
            ..
        } => {
            let value = match tag_encoding {
                TagEncoding::Direct => {
                    layout
                        .ty
                        .discriminant_for_variant(tcx, variant)
                        .unwrap()
                        .val
                }
                TagEncoding::Niche {
                    untagged_variant,
                    niche_variants,
                    niche_start,
                } => {
                    if *untagged_variant == variant {
                        return None;
                    }
                    (variant.as_u32() - niche_variants.start.as_u32()) as u128 + niche_start
                }
            };
            Some((
                layout.fields.offset(tag_field.as_usize()).bytes() as i16,
                tag.size(&tcx).bytes(),
                value,
            ))
        }
        _ => None,
    }
}
