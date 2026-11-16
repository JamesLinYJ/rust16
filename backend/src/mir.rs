use super::layout::{Slots, address, layout, tag_value, transfer};
use rust16::machine::Segment;
use rustc_abi::{TagEncoding, Variants};
use rustc_middle::ty::layout::LayoutCx;
use rustc_middle::{
    mir::*,
    ty::{self, TyCtxt},
};

struct Frame<'a, 'tcx> {
    tcx: TyCtxt<'tcx>,
    body: &'a Body<'tcx>,
    code: &'a mut Segment,
    cpu: u16,
    slots: Slots,
}
impl<'tcx> Frame<'_, 'tcx> {
    // Return the near address in BX. SS and DS alias for this ABI.
    fn addr(&mut self, p: Place<'tcx>) -> Result<(), String> {
        if let [ProjectionElem::Deref, tail @ ..] = p.projection.as_slice() {
            self.code.emit(&[0x8b, 0x9e]);
            self.code
                .word(self.slots.offsets[p.local.as_usize()] as u16);
            let mut ty = self.body.local_decls[p.local]
                .ty
                .builtin_deref(true)
                .ok_or("dereference type")?;
            for proj in tail {
                match proj {
                    ProjectionElem::Field(f, t) => {
                        let off = layout(self.tcx, ty)?.fields.offset(f.as_usize()).bytes();
                        if off != 0 {
                            self.code.emit(&[0x81, 0xc3]);
                            self.code.word(off as u16);
                        }
                        ty = *t;
                    }
                    ProjectionElem::Index(index) => {
                        ty = ty.builtin_index().ok_or("index type")?;
                        let stride = layout(self.tcx, ty)?.size.bytes();
                        self.code.emit(&[0x8b, 0xb6]);
                        self.code.word(self.slots.offsets[index.as_usize()] as u16);
                        match stride {
                            1 => (),
                            2 => self.code.emit(&[0xd1, 0xe6]),
                            4 => self.code.emit(&[0xd1, 0xe6, 0xd1, 0xe6]),
                            _ => return Err("index stride".into()),
                        }
                        self.code.emit(&[0x01, 0xf3]);
                    }
                    ProjectionElem::ConstantIndex {
                        offset,
                        from_end: false,
                        ..
                    } => {
                        ty = ty.builtin_index().ok_or("index type")?;
                        self.code.emit(&[0x81, 0xc3]);
                        self.code
                            .word((offset * layout(self.tcx, ty)?.size.bytes()) as u16);
                    }
                    _ => return Err(format!("unsupported projection {proj:?}")),
                }
            }
        } else {
            let (at, _) = address(self.tcx, self.body, &self.slots, p)?;
            self.code.emit(&[0x8d, 0x9e]);
            self.code.word(at as u16);
        }
        Ok(())
    }
    fn place(&mut self, p: Place<'tcx>, op: u8) -> Result<(), String> {
        if matches!(p.projection.first(), Some(ProjectionElem::Deref)) {
            self.addr(p)?;
            let size = layout(self.tcx, p.ty(self.body, self.tcx).ty)?.size.bytes();
            if size == 0 {
                return Ok(());
            }
            if size > 4 {
                return Err("pointer transfer exceeds register ABI".into());
            }
            if size == 1 && op == 0x8b {
                self.code.emit(&[0x30, 0xe4]);
            }
            self.code.emit(&[if size == 1 { op - 1 } else { op }, 7]);
            if size > 2 {
                if size == 3 && op == 0x8b {
                    self.code.emit(&[0x30, 0xf6]);
                }
                self.code
                    .emit(&[if size == 3 { op - 1 } else { op }, 0x57, 2]);
            }
            return Ok(());
        }
        let (at, data) = address(self.tcx, self.body, &self.slots, p)?;
        transfer(self.code, at, data.size.bytes(), op == 0x89)
    }
    fn immediate(&mut self, bits: u128, bytes: u64) -> Result<(), String> {
        if bytes > 4 {
            return Err(format!("constant wider than four bytes: {bytes}"));
        }
        self.code.emit(&[0xb8]);
        self.code.word(bits as u16);
        if bytes > 2 {
            self.code.emit(&[0xba]);
            self.code.word((bits >> 16) as u16);
        }
        Ok(())
    }
    fn assign(&mut self, p: Place<'tcx>, r: &Rvalue<'tcx>) -> Result<(), String> {
        if let Rvalue::Use(v, _) = r {
            let size = layout(self.tcx, p.ty(self.body, self.tcx).ty)?.size.bytes();
            if size > 4 {
                self.addr(p)?;
                self.code.emit(&[0x89, 0xdf, 0x1e, 0x07]);
                match v {
                    Operand::Copy(src) | Operand::Move(src) => {
                        self.addr(*src)?;
                        self.code.emit(&[0x89, 0xde, 0xb9]);
                        self.code.word(size as u16);
                        self.code.emit(&[0xf3, 0xa4]);
                    }
                    Operand::Constant(c) => {
                        let ConstValue::Indirect { alloc_id, offset } = c
                            .const_
                            .eval(self.tcx, ty::TypingEnv::fully_monomorphized(), c.span)
                            .map_err(|_| "aggregate constant")?
                        else {
                            return Err("missing aggregate bytes".into());
                        };
                        let alloc = self.tcx.global_alloc(alloc_id).unwrap_memory().inner();
                        if !alloc.provenance().ptrs().is_empty() {
                            return Err("relocatable aggregate".into());
                        }
                        let start = offset.bytes_usize();
                        let bytes = alloc.inspect_with_uninit_and_ptr_outside_interpreter(
                            start..start + size as usize,
                        );
                        if bytes.iter().all(|&b| b == 0) {
                            self.code.emit(&[0x31, 0xc0, 0xb9]);
                            self.code.word(size as u16);
                            self.code.emit(&[0xf3, 0xaa]);
                        } else {
                            self.code.emit(&[0x1e, 0x0e, 0x1f, 0xe8]);
                            self.code.word(size as u16);
                            self.code.emit(bytes);
                            self.code.emit(&[0x5e, 0xb9]);
                            self.code.word(size as u16);
                            self.code.emit(&[0xf3, 0xa4, 0x1f]);
                        }
                    }
                    _ => return Err("aggregate source".into()),
                }
                return Ok(());
            }
        }
        if let Rvalue::Aggregate(kind, fields) = r {
            let (at, base) = address(self.tcx, self.body, &self.slots, p)?;
            let cx = LayoutCx::new(self.tcx, ty::TypingEnv::fully_monomorphized());
            let data = match &**kind {
                AggregateKind::Tuple | AggregateKind::Array(_) => base,
                AggregateKind::Adt(_, variant, _, _, None) => {
                    if let Some((off, len, bits)) = tag_value(base, self.tcx, *variant) {
                        self.immediate(bits, len)?;
                        transfer(self.code, at + off, len, true)?;
                    }
                    base.for_variant(&cx, *variant)
                }
                _ => return Err(format!("unsupported aggregate {kind:?}")),
            };
            for (field, operand) in fields.iter_enumerated() {
                self.operand(operand)?;
                let offset = data.fields.offset(field.as_usize()).bytes() as i16;
                transfer(
                    self.code,
                    at + offset,
                    data.field(&cx, field.as_usize()).size.bytes(),
                    true,
                )?;
            }
            return Ok(());
        }
        self.rvalue(r)?;
        self.place(p, 0x89)
    }
    fn literal(&mut self, bytes: &[u8]) {
        self.code.emit(&[0x0e, 0xe8]); // push cs; call past bytes
        self.code.word(bytes.len() as u16);
        self.code.emit(bytes);
        self.code.emit(&[0xb8]);
        self.code.word(bytes.len() as u16);
        self.code.emit(&[0x50]);
        self.code.far_call("rust16_literal");
        self.code.emit(&[0x83, 0xc4, 6]);
    }
    fn operand(&mut self, v: &Operand<'tcx>) -> Result<(), String> {
        match v {
            Operand::Copy(p) | Operand::Move(p) => self.place(*p, 0x8b),
            Operand::Constant(c) => {
                let env = ty::TypingEnv::fully_monomorphized();
                let width = layout(self.tcx, c.const_.ty())?.size.bytes();
                let bits = if let Some(x) = c.const_.try_eval_scalar_int(self.tcx, env) {
                    x.to_bits_unchecked()
                } else {
                    match c
                        .const_
                        .eval(self.tcx, env, c.span)
                        .map_err(|e| format!("constant: {e:?}"))?
                    {
                        ConstValue::ZeroSized => 0,
                        ConstValue::Slice { alloc_id, meta } => {
                            if meta > 0xafa {
                                return Err("literal exceeds DOS near-data pool".into());
                            }
                            let alloc = self.tcx.global_alloc(alloc_id).unwrap_memory().inner();
                            if !alloc.provenance().ptrs().is_empty() {
                                return Err("relocatable slice literal".into());
                            }
                            let element = c
                                .const_
                                .ty()
                                .builtin_deref(true)
                                .ok_or("slice pointer type")?;
                            let stride = match element.kind() {
                                ty::Str => 1,
                                ty::Slice(t) => layout(self.tcx, *t)?.size.bytes(),
                                _ => return Err("unsized literal".into()),
                            };
                            let len = meta
                                .checked_mul(stride)
                                .filter(|n| *n <= 0xafa)
                                .ok_or("literal byte count exceeds pool")?;
                            self.literal(
                                alloc.inspect_with_uninit_and_ptr_outside_interpreter(
                                    0..len as usize,
                                ),
                            );
                            self.code.emit(&[0xba]);
                            self.code.word(meta as u16);
                            return Ok(());
                        }

                        ConstValue::Indirect { alloc_id, offset } if width <= 4 => {
                            let allocation =
                                self.tcx.global_alloc(alloc_id).unwrap_memory().inner();
                            if !allocation.provenance().ptrs().is_empty() {
                                return Err("relocatable constant".into());
                            }
                            let start = offset.bytes_usize();
                            let mut bits = [0; 4];
                            bits[..width as usize].copy_from_slice(
                                allocation.inspect_with_uninit_and_ptr_outside_interpreter(
                                    start..start + width as usize,
                                ),
                            );
                            u32::from_le_bytes(bits) as u128
                        }
                        _ => return Err(format!("unsupported constant {c:?}")),
                    }
                };
                self.immediate(bits, width)
            }
            _ => Err(format!("unsupported operand {v:?}")),
        }
    }
    fn rvalue(&mut self, r: &Rvalue<'tcx>) -> Result<(), String> {
        match r {
            Rvalue::Use(v, _) => self.operand(v)?,
            Rvalue::Ref(_, _, p) | Rvalue::RawPtr(_, p) => {
                self.addr(*p)?;
                self.code.emit(&[0x89, 0xd8]);
            }
            Rvalue::Discriminant(p) => {
                let (at, data) = address(self.tcx, self.body, &self.slots, *p)?;
                match &data.variants {
                    Variants::Single { index } => self.immediate(
                        data.ty
                            .discriminant_for_variant(self.tcx, *index)
                            .unwrap()
                            .val,
                        2,
                    )?,
                    Variants::Multiple {
                        tag,
                        tag_encoding,
                        tag_field,
                        ..
                    } => {
                        let width = tag.size(&self.tcx).bytes();
                        if width > 2 {
                            return Err("wide discriminant".into());
                        }
                        transfer(
                            self.code,
                            at + data.fields.offset(tag_field.as_usize()).bytes() as i16,
                            width,
                            false,
                        )?;
                        if let TagEncoding::Niche {
                            untagged_variant,
                            niche_variants,
                            niche_start,
                        } = tag_encoding
                        {
                            let first = niche_variants.start.as_u32() as u16;
                            let last = niche_variants.last.as_u32() as u16;
                            self.code.emit(&[0x2d]);
                            self.code.word(*niche_start as u16);
                            if width == 1 {
                                self.code.emit(&[0x30, 0xe4]);
                            }
                            self.code.emit(&[0x3d]);
                            self.code.word(last - first);
                            self.code.emit(&[0x76, 5, 0xb8]);
                            self.code.word(untagged_variant.as_u32() as u16);
                            self.code.emit(&[0xeb, 3, 0x05]);
                            self.code.word(first);
                        }
                    }
                    _ => return Err("uninhabited discriminant".into()),
                }
            }
            Rvalue::Cast(CastKind::IntToInt, val, to) => {
                self.operand(val)?;
                let from = val.ty(self.body, self.tcx);
                let width = layout(self.tcx, from)?.size.bytes();
                let dest = layout(self.tcx, *to)?.size.bytes();
                if dest > 4 {
                    return Err("wide integer cast".into());
                }
                if width == 1 && dest > 1 && from.is_signed() {
                    self.code.emit(&[0x98]);
                }
                if width <= 2 && dest > 2 {
                    if from.is_signed() {
                        self.code.emit(&[0x99]);
                    } else {
                        self.code.emit(&[0x31, 0xd2]);
                    }
                }
            }
            Rvalue::BinaryOp(op, operands) => {
                let (a, b) = &**operands;
                let aty = a.ty(self.body, self.tcx);
                let width = layout(self.tcx, aty)?.size.bytes();
                if width > 2 {
                    if width != 4 {
                        return Err("wide arithmetic".into());
                    }
                    self.operand(a)?;
                    self.code.emit(&[0x52, 0x50]);
                    self.operand(b)?;
                    self.code.emit(&[0x89, 0xd1, 0x89, 0xc3, 0x58, 0x5a]);
                    match op {
                        BinOp::BitAnd => self.code.emit(&[0x21, 0xd8, 0x21, 0xca]),
                        BinOp::BitOr => self.code.emit(&[0x09, 0xd8, 0x09, 0xca]),
                        BinOp::BitXor => self.code.emit(&[0x31, 0xd8, 0x31, 0xca]),
                        BinOp::Mul | BinOp::MulUnchecked => {
                            // Low 32 bits of (DX:AX * CX:BX), using three 16-bit products.
                            self.code.emit(&[
                                0x89, 0xc6, 0x89, 0xd0, 0xf7, 0xe3, 0x89, 0xc7, 0x89, 0xf0, 0xf7,
                                0xe1, 0x01, 0xc7, 0x89, 0xf0, 0xf7, 0xe3, 0x01, 0xfa,
                            ]);
                        }
                        BinOp::Add | BinOp::AddUnchecked => {
                            self.code.emit(&[0x01, 0xd8, 0x11, 0xca])
                        }
                        BinOp::Sub | BinOp::SubUnchecked => {
                            self.code.emit(&[0x29, 0xd8, 0x19, 0xca])
                        }
                        BinOp::Shl | BinOp::ShlUnchecked | BinOp::Shr | BinOp::ShrUnchecked => {
                            self.code.emit(&[0x89, 0xd9, 0x83, 0xe1, 31, 0xe3, 6]);
                            if matches!(op, BinOp::Shl | BinOp::ShlUnchecked) {
                                self.code.emit(&[0xd1, 0xe0, 0xd1, 0xd2]);
                            } else {
                                self.code.emit(&[
                                    0xd1,
                                    if aty.is_signed() { 0xfa } else { 0xea },
                                    0xd1,
                                    0xd8,
                                ]);
                            }
                            self.code.emit(&[0xe2, 0xfa]);
                        }
                        BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Ge | BinOp::Gt => {
                            let pos = self.code.bytes.len();
                            let yes = format!("comparison/{pos}/yes");
                            let no = format!("comparison/{pos}/no");
                            let done = format!("comparison/{pos}/done");
                            let lower = if aty.is_signed() { 0x7c } else { 0x72 };
                            let higher = if aty.is_signed() { 0x7f } else { 0x77 };
                            self.code.emit(&[0x39, 0xca]); // cmp dx,cx
                            match op {
                                BinOp::Eq => self.code.branch(0x75, &no),
                                BinOp::Ne => self.code.branch(0x75, &yes),
                                BinOp::Lt | BinOp::Le => {
                                    self.code.branch(lower, &yes);
                                    self.code.branch(higher, &no);
                                }
                                _ => {
                                    self.code.branch(higher, &yes);
                                    self.code.branch(lower, &no);
                                }
                            }
                            self.code.emit(&[0x39, 0xd8]);
                            self.code.branch(
                                match op {
                                    BinOp::Eq => 0x74,
                                    BinOp::Ne => 0x75,
                                    BinOp::Lt => 0x72,
                                    BinOp::Le => 0x76,
                                    BinOp::Gt => 0x77,
                                    _ => 0x73,
                                },
                                &yes,
                            );
                            self.code.label(&no);
                            self.code.emit(&[0x31, 0xc0]);
                            self.code.relative(0xe9, &done);
                            self.code.label(&yes);
                            self.code.emit(&[0xb8, 1, 0]);
                            self.code.label(&done);
                        }
                        _ => return Err(format!("unsupported 32-bit operation {op:?}")),
                    }
                    return Ok(());
                }
                self.operand(a)?;
                if aty.is_signed() && width == 1 {
                    self.code.emit(&[0x98]);
                }
                self.code.emit(&[0x50]);
                self.operand(b)?;
                if b.ty(self.body, self.tcx).is_signed()
                    && layout(self.tcx, b.ty(self.body, self.tcx))?.size.bytes() == 1
                {
                    self.code.emit(&[0x98]);
                }
                self.code.emit(&[0x89, 0xc1, 0x58]);
                use BinOp::*;
                match op {
                    Add | AddUnchecked => self.code.emit(&[1, 0xc8]),
                    Sub | SubUnchecked => self.code.emit(&[0x29, 0xc8]),
                    Mul | MulUnchecked => self.code.emit(&[0xf7, 0xe1]),
                    BitAnd => self.code.emit(&[0x21, 0xc8]),
                    BitOr => self.code.emit(&[0x09, 0xc8]),
                    BitXor => self.code.emit(&[0x31, 0xc8]),
                    Shl | ShlUnchecked | Shr | ShrUnchecked => self.code.emit(&[
                        0x80,
                        0xe1,
                        (width * 8 - 1) as u8,
                        0xd3,
                        if matches!(op, Shl | ShlUnchecked) {
                            0xe0
                        } else {
                            if aty.is_signed() { 0xf8 } else { 0xe8 }
                        },
                    ]),
                    Div | Rem => {
                        if aty.is_signed() {
                            self.code.emit(&[0x99, 0xf7, 0xf9]);
                        } else {
                            self.code.emit(&[0x31, 0xd2, 0xf7, 0xf1]);
                        }
                        if *op == Rem {
                            self.code.emit(&[0x89, 0xd0]);
                        }
                    }
                    Eq | Ne | Lt | Gt | Le | Ge => {
                        let cc = match op {
                            Eq => 4,
                            Ne => 5,
                            Lt if aty.is_signed() => 12,
                            Le if aty.is_signed() => 14,
                            Gt if aty.is_signed() => 15,
                            Ge if aty.is_signed() => 13,
                            Lt => 2,
                            Gt => 7,
                            Le => 6,
                            _ => 3,
                        };
                        self.code.emit(&[0x39, 0xc8]);
                        if self.cpu == 386 {
                            self.code.emit(&[0x0f, 0x90 + cc, 0xc0, 0x30, 0xe4]);
                        } else {
                            self.code
                                .emit(&[0x70 + cc, 4, 0x31, 0xc0, 0xeb, 3, 0xb8, 1, 0]);
                        }
                    }
                    _ => return Err(format!("unsupported binary operation {op:?}")),
                }
            }
            Rvalue::UnaryOp(UnOp::PtrMetadata, v) => {
                self.operand(v)?;
                self.code.emit(&[0x89, 0xd0]);
            }
            Rvalue::Cast(CastKind::Transmute | CastKind::PtrToPtr, v, ty)
                if layout(self.tcx, *ty)?.size
                    == layout(self.tcx, v.ty(self.body, self.tcx))?.size =>
            {
                self.operand(v)?
            }
            Rvalue::UnaryOp(UnOp::Not, v) => {
                self.operand(v)?;
                if v.ty(self.body, self.tcx).is_bool() {
                    self.code.emit(&[0x83, 0xf0, 1]);
                } else {
                    self.code.emit(&[0xf7, 0xd0]);
                    if layout(self.tcx, v.ty(self.body, self.tcx))?.size.bytes() == 4 {
                        self.code.emit(&[0xf7, 0xd2]);
                    }
                }
            }
            _ => return Err(format!("unsupported MIR value {r:?}")),
        }
        Ok(())
    }
}
pub fn function<'tcx>(
    tcx: TyCtxt<'tcx>,
    body: &Body<'tcx>,
    name: &str,
    cpu: u16,
    s: &mut Segment,
    queue: &mut Vec<ty::Instance<'tcx>>,
) -> Result<(), String> {
    let slots = Slots::new(tcx, body)?;
    let size = slots.frame;
    s.public(name);
    if cpu >= 286 {
        s.emit(&[0xc8]);
        s.word(size);
        s.emit(&[0]);
    } else {
        s.emit(&[0x55, 0x89, 0xe5, 0x81, 0xec]);
        s.word(size);
    }
    let label = |bb: BasicBlock| format!("{name}/{}", bb.as_usize());
    let mut f = Frame {
        tcx,
        slots,
        body,
        code: s,
        cpu,
    };
    for (bb, data) in body.basic_blocks.iter_enumerated() {
        f.code.label(&label(bb));
        for stmt in &data.statements {
            match &stmt.kind {
                StatementKind::Assign(stmt) => {
                    let (p, r) = &**stmt;
                    f.assign(*p, r)?;
                }
                StatementKind::StorageLive(_)
                | StatementKind::StorageDead(_)
                | StatementKind::Nop => (),
                k => return Err(format!("unsupported MIR statement {k:?}")),
            }
        }
        match &data.terminator().kind {
            TerminatorKind::Return => {
                if !body.return_ty().is_unit() {
                    f.place(Place::from(RETURN_PLACE), 0x8b)?;
                }
                if cpu >= 286 {
                    f.code.emit(&[0xc9]);
                } else {
                    f.code.emit(&[0x89, 0xec, 0x5d]);
                }
                f.code.emit(&[0xcb]);
            }
            TerminatorKind::Goto { target, .. } => f.code.relative(0xe9, &label(*target)),
            TerminatorKind::SwitchInt { discr, targets } => {
                f.operand(discr)?;
                let width = layout(tcx, discr.ty(body, tcx))?.size.bytes();
                for (val, dest) in targets.iter() {
                    let next = format!("switch/{}", f.code.bytes.len());
                    if width > 2 {
                        if width != 4 {
                            return Err("switch exceeds 32 bits".into());
                        }
                        f.code.emit(&[0x81, 0xfa]);
                        f.code.word((val >> 16) as u16);
                        f.code.branch(0x75, &next);
                    }
                    f.code.emit(&[0x3d]);
                    f.code.word(val as u16);
                    f.code.branch(0x74, &label(dest));
                    if width > 2 {
                        f.code.label(&next);
                    }
                }
                f.code.relative(0xe9, &label(targets.otherwise()));
            }
            TerminatorKind::Call {
                func,
                args,
                destination,
                target,
                ..
            } => {
                let (id, substs) = func.const_fn_def().ok_or("indirect call unsupported")?;
                let instance = ty::Instance::try_resolve(
                    tcx,
                    ty::TypingEnv::fully_monomorphized(),
                    id,
                    substs,
                )
                .map_err(|_| format!("cannot resolve {id:?}"))?
                .ok_or_else(|| format!("ambiguous call {id:?}"))?;
                if !tcx.is_foreign_item(instance.def_id()) {
                    queue.push(instance);
                }
                let mut pushed = 0;
                for arg in args {
                    let size = layout(tcx, arg.node.ty(body, tcx))?.size.bytes();
                    if size == 0 {
                        continue;
                    }
                    if size > 4 {
                        return Err("argument exceeds register ABI".into());
                    }
                    pushed += 2 * size.div_ceil(2);
                    f.operand(&arg.node)?;
                    if size > 2 {
                        f.code.emit(&[0x52]);
                    }
                    f.code.emit(&[0x50]);
                }
                f.code.far_call(&super::symbol(tcx, instance));
                if pushed != 0 {
                    f.code.emit(&[0x81, 0xc4]);
                    f.code.word(pushed as u16);
                }
                if let Some(dest) = target {
                    if !destination.ty(body, tcx).ty.is_unit() {
                        f.place(*destination, 0x89)?;
                    }
                    f.code.relative(0xe9, &label(*dest));
                } else {
                    f.code.far_call("rust16_abort");
                }
            }
            TerminatorKind::Assert {
                cond,
                expected,
                target,
                ..
            } => {
                f.operand(cond)?;
                f.code.emit(&[0x09, 0xc0]);
                f.code
                    .branch(if *expected { 0x75 } else { 0x74 }, &label(*target));
                f.code.far_call("rust16_abort");
            }
            TerminatorKind::Unreachable => f.code.far_call("rust16_abort"),
            k => return Err(format!("unsupported MIR terminator {k:?}")),
        }
    }
    Ok(())
}
