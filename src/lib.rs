//! Host implementation of the byte workspace and DOS-facing compiler services.
//! The same compiler and linker sources are compiled to DOS by the seed.
pub mod firmware;
mod literals;
mod bios_disk;
pub mod machine;
pub mod runtime;
use std::cell::RefCell;
use std::fs;
#[derive(Default)]
struct Context {
    args: Vec<String>,
    banks: Vec<Vec<u8>>,
}
thread_local! {static CTX: RefCell<Context> = RefCell::new(Context::default());}
fn valid(bank: u16, index: u16) {
    if bank > 9 || (!matches!(bank, 0 | 1 | 6) && index >= 16384) {
        fail(12);
    }
}
fn fetch(bank: u16, index: u16) -> u16 {
    valid(bank, index);
    CTX.with_borrow(|c| {
        c.banks
            .get(bank as usize)
            .and_then(|p| p.get(index as usize))
            .copied()
            .unwrap_or(0) as u16
    })
}
fn save(bank: u16, index: u16, value: u16) -> u16 {
    valid(bank, index);
    CTX.with_borrow_mut(|c| {
        let b = &mut c.banks[bank as usize];
        if b.len() <= index as usize {
            b.resize(index as usize + 1, 0);
        }
        b[index as usize] = value as u8;
    });
    value
}
fn arg(index: u16, pos: u16) -> u16 {
    CTX.with_borrow(|c| {
        c.args
            .get(index as usize)
            .and_then(|s| s.as_bytes().get(pos as usize))
            .copied()
            .unwrap_or(0) as u16
    })
}
fn path(n: u16) -> String {
    CTX.with_borrow(|c| c.args.get(n as usize).cloned())
        .unwrap_or_else(|| fail(10))
}
fn input(n: u16, b: u16) -> u16 {
    let p = path(n);
    let data = fs::read(&p).unwrap_or_else(|e| {
        eprintln!("{p}: {e}");
        fail(11)
    });
    valid(b, 0);
    let limit = if matches!(b, 0 | 1 | 6) { 65535 } else { 16383 };
    if data.len() >= limit {
        fail(12)
    }
    let len = data.len() as u16;
    CTX.with_borrow_mut(|c| c.banks[b as usize] = data);
    len
}
fn output(n: u16, b: u16, len: u16) -> u16 {
    let p = path(n);
    valid(b, len.saturating_sub(1));
    CTX.with_borrow_mut(|c| {
        let bank = &mut c.banks[b as usize];
        if bank.len() < usize::from(len) {
            bank.resize(usize::from(len), 0);
        }
        fs::write(&p, &bank[..usize::from(len)])
    })
    .unwrap_or_else(|e| {
        eprintln!("{p}: {e}");
        fail(13)
    });
    0
}
fn putch(v: u16) -> u16 {
    use std::io::Write;
    std::io::stdout().write_all(&[v as u8]).unwrap();
    v
}
fn fail(v: u16) -> ! {
    eprintln!("rust16 error {v}");
    std::process::exit(i32::from(v))
}
pub fn run(args: Vec<String>, compiler: bool) -> u16 {
    CTX.with_borrow_mut(|c| {
        *c = Context {
            args,
            banks: vec![vec![]; 32],
        }
    });
    if compiler {
        compiler::main16()
    } else {
        linker::main16()
    }
}
#[allow(
    unused_mut,
    unused_parens,
    unused_assignments,
    dead_code,
    unreachable_code
)]
mod compiler {
    use super::*;
    include!("../dos/common.rs");
    include!("../dos/compile.rs");
    pub(super) fn pack(s: &super::machine::Segment) -> Result<Vec<u8>, String> {
        use std::collections::BTreeSet;
        let mut exported = BTreeSet::new();
        for (name, _) in &s.publics {
            if !exported.insert(name) {
                return Err(format!("duplicate public {name}"));
            }
        }
        if s.publics
            .iter()
            .map(|v| &v.0)
            .chain(s.calls.iter().map(|v| &v.1))
            .any(|n| n.is_empty() || n.len() > 127)
        {
            return Err("invalid OMF symbol name".into());
        }
        CTX.with_borrow_mut(|c| {
            *c = Context {
                args: vec![],
                banks: vec![vec![]; 32],
            }
        });
        fn sym(n: &str) -> u16 {
            for (i, b) in n.bytes().enumerate() {
                save(0, i as u16, b.into());
            }
            intern(0, 0, n.len() as u16)
        }
        for (n, ofs) in &s.publics {
            set(4, *ofs as u16);
            define(sym(n));
        }
        for (i, (ofs, n)) in s.calls.iter().enumerate() {
            setw(5, i as u16 * 4, *ofs as u16);
            setw(5, i as u16 * 4 + 2, sym(n));
        }
        for (i, v) in s.bytes.iter().enumerate() {
            save(1, i as u16, u16::from(*v));
        }
        set(4, s.bytes.len() as u16);
        set(6, s.calls.len() as u16);
        obj();
        Ok(CTX.with_borrow(|c| c.banks[6].clone()))
    }
}
#[allow(
    unused_mut,
    unused_parens,
    unused_assignments,
    dead_code,
    unreachable_code
)]
mod linker {
    use super::*;
    include!("../dos/common.rs");
    include!("../dos/link.rs");
}
