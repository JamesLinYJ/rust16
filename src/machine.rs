//! One paragraph-aligned 16-bit code segment, with OMF external far references.
use std::collections::BTreeMap;
#[derive(Default)]
pub struct Segment {
    pub bytes: Vec<u8>,
    pub publics: Vec<(String, usize)>,
    pub calls: Vec<(usize, String)>,
    labels: BTreeMap<String, usize>,
    branches: Vec<(usize, String)>,
}
impl Segment {
    pub fn emit(&mut self, bytes: &[u8]) {
        self.bytes.extend_from_slice(bytes);
    }
    pub fn word(&mut self, v: u16) {
        self.emit(&v.to_le_bytes());
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
        self.branches.push((self.bytes.len(), label.into()));
        self.word(0);
    }
    pub fn branch(&mut self, cc: u8, label: &str) {
        self.emit(&[cc ^ 1, 3]);
        self.relative(0xe9, label);
    }
    pub fn far_call(&mut self, name: &str) {
        self.emit(&[0x9a]);
        self.calls.push((self.bytes.len(), name.into()));
        self.emit(&[0; 4]);
    }
    pub fn finish(mut self) -> Result<Vec<u8>, String> {
        if self.bytes.len() > 59900 || self.calls.len() > 3900 {
            return Err("segment exceeds OMF writer limits".into());
        }
        for (p, name) in self.branches.drain(..) {
            let dest = *self
                .labels
                .get(&name)
                .ok_or_else(|| format!("unknown label {name}"))?;
            self.bytes[p..p + 2]
                .copy_from_slice(&((dest as u16).wrapping_sub(p as u16 + 2)).to_le_bytes());
        }
        super::compiler::pack(&self)
    }
}
