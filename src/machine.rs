//! One paragraph-aligned 16-bit code segment, with OMF external far references.
use std::collections::BTreeMap;
#[derive(Default)]
pub struct Segment {
    pub bytes: Vec<u8>,
    pub publics: Vec<(String, usize)>,
    pub calls: Vec<(usize, String)>,
    labels: BTreeMap<String, usize>,
    branches: Vec<(usize, String, Option<u8>)>,
}
impl Segment {
    pub fn emit(&mut self, bytes: &[u8]) {
        self.bytes.extend_from_slice(bytes);
    }
    pub fn word(&mut self, v: u16) {
        self.emit(&v.to_le_bytes());
    }
    /// Encode a 16-bit [BP+disp] operand, including [BP] as disp8=0.
    pub fn bp_operand(&mut self, opcode: u8, reg: u8, offset: i16) {
        if let Ok(short) = i8::try_from(offset) {
            self.emit(&[opcode, 0x46 | reg << 3, short as u8]);
        } else {
            self.emit(&[opcode, 0x86 | reg << 3]);
            self.word(offset as u16);
        }
    }
    /// Group 1 sign-extends imm8 before the same word operation.
    pub fn arithmetic_word(&mut self, modrm: u8, value: u16) {
        if let Ok(short) = i8::try_from(value as i16) {
            self.emit(&[0x83, modrm, short as u8]);
        } else {
            self.emit(&[0x81, modrm]);
            self.word(value);
        }
    }
    pub fn public(&mut self, name: &str) {
        self.publics.push((name.into(), self.bytes.len()));
    }
    pub fn label(&mut self, name: &str) {
        assert!(self.labels.insert(name.into(), self.bytes.len()).is_none());
    }
    pub fn label_offset(&self, name: &str) -> usize {
        self.labels[name]
    }
    pub fn relative(&mut self, opcode: u8, label: &str) {
        self.emit(&[opcode]);
        self.branches.push((self.bytes.len(), label.into(), None));
        self.word(0);
    }
    pub fn branch(&mut self, cc: u8, label: &str) {
        self.emit(&[cc ^ 1, 3]);
        self.relative(0xe9, label);
        self.branches.last_mut().unwrap().2 = Some(cc);
    }
    pub fn far_call(&mut self, name: &str) {
        self.emit(&[0x9a]);
        self.calls.push((self.bytes.len(), name.into()));
        self.emit(&[0; 4]);
    }
    fn fix_branches(&mut self) -> Result<(), String> {
        for (p, name, _) in self.branches.drain(..) {
            let dest = *self
                .labels
                .get(&name)
                .ok_or_else(|| format!("unknown label {name}"))?;
            self.bytes[p..p + 2]
                .copy_from_slice(&((dest as u16).wrapping_sub(p as u16 + 2)).to_le_bytes());
        }
        Ok(())
    }
    pub fn resolve(mut self) -> Result<Vec<u8>, String> {
        self.fix_branches()?;
        Ok(self.bytes)
    }
    /// Relax MIR branches with known targets before packing OMF. Literal data
    /// and inline instruction sequences remain opaque. Only recorded label
    /// fixups and external relocations change position.
    pub fn relax_branches(&mut self) -> Result<(), String> {
        struct Jump {
            start: usize,
            end: usize,
            target: usize,
            condition: Option<u8>,
            length: usize,
        }
        let mut jumps = Vec::new();
        for (operand, label, condition) in &self.branches {
            if self.bytes[operand - 1] != 0xe9 {
                return Err("MIR branch relaxation requires a jump fixup".into());
            }
            let length = if condition.is_some() { 5 } else { 3 };
            let end = operand + 2;
            let target = *self
                .labels
                .get(label)
                .ok_or_else(|| format!("unknown label {label}"))?;
            jumps.push(Jump {
                start: end - length,
                end,
                target,
                condition: *condition,
                length,
            });
        }
        // The cumulative reduction applies after each whole old instruction.
        // A candidate forward jump shortens both its end and its target by
        // the same amount. For a backward jump, test against its new end.
        let mut reductions = Vec::new();
        let offset = |pos: usize, reductions: &[(usize, usize)]| {
            let n = reductions.partition_point(|&(end, _)| end <= pos);
            pos - if n == 0 { 0 } else { reductions[n - 1].1 }
        };
        loop {
            reductions.clear();
            let mut removed = 0;
            for j in &jumps {
                removed += j.end - j.start - j.length;
                reductions.push((j.end, removed));
            }
            let mut changed = false;
            for j in &mut jumps {
                let start = offset(j.start, &reductions) as isize;
                let target = offset(j.target, &reductions) as isize;
                let length = if j.target >= j.end && target == start + j.length as isize {
                    0
                } else {
                    let end = start
                        + if j.target >= j.end {
                            j.length as isize
                        } else {
                            2
                        };
                    if i8::try_from(target - end).is_ok() {
                        2
                    } else {
                        j.length
                    }
                };
                if length < j.length {
                    j.length = length;
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
        let mut bytes = Vec::with_capacity(offset(self.bytes.len(), &reductions));
        let mut prev = 0;
        for j in jumps {
            bytes.extend_from_slice(&self.bytes[prev..j.start]);
            let delta = offset(j.target, &reductions) as isize - (bytes.len() + j.length) as isize;
            if j.length == 2 {
                bytes.extend_from_slice(&[j.condition.unwrap_or(0xeb), delta as i8 as u8]);
            } else if j.length != 0 {
                if let Some(cc) = j.condition {
                    bytes.extend_from_slice(&[cc ^ 1, 3]);
                }
                bytes.push(0xe9);
                bytes.extend_from_slice(&(delta as i16).to_le_bytes());
            }
            prev = j.end;
        }
        bytes.extend_from_slice(&self.bytes[prev..]);
        for (_, p) in &mut self.publics {
            *p = offset(*p, &reductions);
        }
        for (p, _) in &mut self.calls {
            *p = offset(*p, &reductions);
        }
        for p in self.labels.values_mut() {
            *p = offset(*p, &reductions);
        }
        self.branches.clear();
        self.bytes = bytes;
        Ok(())
    }
    pub fn finish_mir(mut self) -> Result<Vec<u8>, String> {
        self.relax_branches()?;
        self.finish()
    }
    pub fn finish(mut self) -> Result<Vec<u8>, String> {
        if self.bytes.len() > 59900 || self.calls.len() > 3900 {
            return Err("segment exceeds OMF writer limits".into());
        }
        self.fix_branches()?;
        super::compiler::pack(&self)
    }
}
