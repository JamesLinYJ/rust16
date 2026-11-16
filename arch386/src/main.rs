//! CPU and calling-convention adapters for segmented DOS device images.
//! The monitor itself is an independently linked i386 Rust image at offset 0x400.
use iced_x86::{Code, IcedError, Instruction, MemoryOperand, Register, code_asm::*};
use std::{env, fs};
type Result<T> = std::result::Result<T, IcedError>;

fn caller() -> Result<Vec<u8>> {
    let mut a = CodeAssembler::new(16)?;
    a.push(bp)?;
    a.mov(bp, sp)?;
    a.pushf()?;
    a.push(ds)?;
    a.push(es)?;
    a.pusha()?;
    a.mov(es, word_ptr(bp + 8))?;
    a.mov(bx, word_ptr(bp + 6))?;
    a.add_instruction(Instruction::with1(
        Code::Call_m1616,
        MemoryOperand::with_base_displ(Register::BP, 10),
    )?)?;
    a.popa()?;
    a.pop(es)?;
    a.pop(ds)?;
    a.popf()?;
    a.pop(bp)?;
    a.retf()?;
    let mut s = rust16::machine::Segment::default();
    s.public("device_call");
    s.emit(&a.assemble(0)?);
    Ok(s.finish().expect("encode DOS device-call OMF"))
}

fn place(blob: &mut [u8], at: usize, next: usize, a: &mut CodeAssembler) -> Result<()> {
    let bytes = a.assemble(at as u64)?;
    assert!(at + bytes.len() <= next, "device entry overlap at {at:#x}");
    blob[at..at + bytes.len()].copy_from_slice(&bytes);
    Ok(())
}

const PACKET: i32 = 0x300;
const LOAD_SEG: i32 = 0x304;
const REAL_SS: i32 = 0x306;
const REAL_SP: i32 = 0x308;
const EXT_KIB: i32 = 0x30a;
const GDTR: i32 = 0x310;
const GDT: i32 = 0x320;
const IRET_ENTRY: usize = 0x20;
const STRATEGY: usize = 0x40;
const INTERRUPT: usize = 0x60;
const RESUME: usize = 0x200;
const PROTECTED: usize = 0x240;
const STACK_TOP: u32 = 0xd000;

fn image() -> Result<Vec<u8>> {
    let mut b = vec![0u8; 0x400];
    b[..4].copy_from_slice(&u32::MAX.to_le_bytes());
    b[4..6].copy_from_slice(&0x8000u16.to_le_bytes());
    b[6..8].copy_from_slice(&(STRATEGY as u16).to_le_bytes());
    b[8..10].copy_from_slice(&(INTERRUPT as u16).to_le_bytes());
    b[10..18].copy_from_slice(b"EMMXXXX0");
    b[IRET_ENTRY + 10..IRET_ENTRY + 18].copy_from_slice(b"EMMXXXX0");
    let mut a = CodeAssembler::new(16)?;
    a.int(0xe6)?;
    a.iret()?;
    place(&mut b, IRET_ENTRY, IRET_ENTRY + 10, &mut a)?;
    let mut a = CodeAssembler::new(16)?;
    a.mov(word_ptr(PACKET).cs(), bx)?;
    a.mov(word_ptr(PACKET + 2).cs(), es)?;
    a.retf()?;
    place(&mut b, STRATEGY, INTERRUPT, &mut a)?;
    let mut a = CodeAssembler::new(16)?;
    let mut skip = a.create_label();
    let mut done = a.create_label();
    let mut init = a.create_label();
    let mut supported = a.create_label();
    a.pushf()?;
    a.push(ds)?;
    a.push(es)?;
    a.pusha()?;
    a.push(cs)?;
    a.pop(ds)?;
    a.les(bx, dword_ptr(PACKET))?;
    a.cmp(byte_ptr(bx + 2).es(), 0)?;
    a.je(init)?;
    a.mov(word_ptr(bx + 3).es(), 0x8103)?;
    a.jmp(done)?;
    a.set_label(&mut init)?;
    // On a 286, the high FLAGS bits cannot be set in real mode.
    a.pushf()?;
    a.pop(ax)?;
    a.mov(cx, ax)?;
    a.or(ax, 0x3000)?;
    a.push(ax)?;
    a.popf()?;
    a.pushf()?;
    a.pop(ax)?;
    a.push(cx)?;
    a.popf()?;
    a.test(ax, 0x3000)?;
    a.jne(supported)?;
    // An 80286 cannot decode a near two-byte Jcc. Use JMP for the long branch.
    a.mov(word_ptr(bx + 14).es(), 0)?;
    a.mov(word_ptr(bx + 16).es(), cs)?;
    a.mov(word_ptr(bx + 3).es(), 0x100)?;
    a.jmp(done)?;
    a.set_label(&mut supported)?;
    // A second monitor may already own CR3 and the interrupt controller.
    a.smsw(ax)?;
    a.test(ax, 1)?;
    a.jne(skip)?;
    // Reject adapters whose BIOS ROM occupies any part of the D000h page frame.
    let mut scan = a.create_label();
    let mut advance = a.create_label();
    a.mov(cx, 0xc000)?;
    a.set_label(&mut scan)?;
    a.mov(es, cx)?;
    a.cmp(word_ptr(0).es(), 0xaa55)?;
    a.jne(advance)?;
    a.cmp(cx, 0xd000)?;
    a.jae(skip)?;
    a.xor(ax, ax)?;
    a.mov(al, byte_ptr(2).es())?;
    a.shl(ax, 5)?;
    a.add(ax, cx)?;
    a.jb(skip)?;
    a.cmp(ax, 0xd000)?;
    a.ja(skip)?;
    a.set_label(&mut advance)?;
    a.add(cx, 0x80)?;
    a.cmp(cx, 0xe000)?;
    a.jb(scan)?;
    a.les(bx, dword_ptr(PACKET))?;
    a.mov(ax, cs)?;
    a.test(ax, 255)?;
    a.jne(skip)?; // physical paging structures align to 4 KiB
    a.add(ax, (STACK_TOP / 16) as i32)?;
    a.cmp(ax, word_ptr(bx + 16).es())?;
    a.ja(skip)?;
    a.mov(ah, 0x88)?;
    a.int(0x15)?;
    a.jb(skip)?;
    a.cmp(ax, 128)?;
    a.jb(skip)?;
    a.mov(word_ptr(EXT_KIB), ax)?;
    // Prefer the BIOS A20 service. Verify using reversible writes at two alias addresses.
    a.mov(ax, 0x2401)?;
    a.int(0x15)?;
    a.cli()?;
    let mut a20_test = a.create_label();
    let mut a20_ok = a.create_label();
    a.call(a20_test)?;
    a.jne(a20_ok)?;
    a.in_(al, 0x92)?;
    a.and(al, 0xfe)?;
    a.or(al, 2)?;
    a.out(0x92, al)?;
    a.call(a20_test)?;
    a.je(skip)?;
    a.set_label(&mut a20_ok)?;
    a.les(bx, dword_ptr(PACKET))?;
    a.mov(ax, cs)?;
    a.mov(word_ptr(LOAD_SEG), ax)?;
    a.mov(word_ptr(REAL_SS), ss)?;
    a.mov(word_ptr(REAL_SP), sp)?;
    a.add(ax, (STACK_TOP / 16) as i32)?;
    a.mov(word_ptr(bx + 14).es(), 0)?;
    a.mov(word_ptr(bx + 16).es(), ax)?;
    a.mov(word_ptr(bx + 3).es(), 0x100)?;

    // Build physical GDT pointers from the DOS load paragraph.
    a.xor(eax, eax)?;
    a.mov(ax, cs)?;
    a.shl(eax, 4)?;
    for entry in [GDT + 8, GDT + 16] {
        a.mov(word_ptr(entry + 2), ax)?;
    }
    a.mov(edx, eax)?;
    a.shr(edx, 16)?;
    for entry in [GDT + 8, GDT + 16] {
        a.mov(byte_ptr(entry + 4), dl)?;
    }
    a.add(eax, GDT)?;
    a.mov(dword_ptr(GDTR + 2), eax)?;
    a.lgdt(fword_ptr(GDTR))?;
    a.mov(eax, cr0)?;
    a.or(eax, 1)?;
    a.mov(cr0, eax)?;
    a.jmp_far(8, PROTECTED as u32)?;
    a.set_label(&mut skip)?;
    a.les(bx, dword_ptr(PACKET))?;
    a.mov(word_ptr(bx + 14).es(), 0)?;
    a.mov(word_ptr(bx + 16).es(), cs)?;
    a.mov(word_ptr(bx + 3).es(), 0x8103)?;
    a.jmp(done)?;

    a.set_label(&mut done)?;
    a.popa()?;
    a.pop(es)?;
    a.pop(ds)?;
    a.popf()?;
    a.retf()?;
    a.set_label(&mut a20_test)?;
    a.push(ds)?;
    a.xor(ax, ax)?;
    a.mov(ds, ax)?;
    a.dec(ax)?;
    a.mov(es, ax)?;
    a.mov(al, byte_ptr(0x500))?;
    a.mov(ah, byte_ptr(0x510).es())?;
    a.mov(byte_ptr(0x500), 0)?;
    a.mov(byte_ptr(0x510).es(), 0xff)?;
    a.cmp(byte_ptr(0x500), 0xff)?;
    a.mov(byte_ptr(0x510).es(), ah)?;
    a.mov(byte_ptr(0x500), al)?;
    a.pop(ds)?;
    a.ret()?;
    place(&mut b, INTERRUPT, RESUME, &mut a)?;
    let mut a = CodeAssembler::new(16)?;
    a.popa()?;
    a.pop(es)?;
    a.pop(ds)?;
    a.popf()?;
    a.retf()?;
    place(&mut b, RESUME, PROTECTED, &mut a)?;
    let mut a = CodeAssembler::new(32)?;
    a.mov(ax, 16)?;
    a.mov(ds, ax)?;
    a.mov(es, ax)?;
    a.mov(ss, ax)?;
    a.mov(esp, STACK_TOP)?;
    // Real firmware may leave NT set. The monitor starts a fresh task with IF clear.
    a.push(2)?;
    a.popfd()?;
    a.cld()?;
    a.call(0x400u64)?;
    a.ud2()?;
    place(&mut b, PROTECTED, 0x270, &mut a)?;
    // Interrupts can arrive with the DOS direction flag set. The saved V86 flags
    // are restored by IRET, while the Rust handler always sees the i386 C ABI.
    let mut a = CodeAssembler::new(32)?;
    a.cld()?;
    a.jmp(dword_ptr(0x348))?;
    place(&mut b, 0x270, PACKET as usize, &mut a)?;
    b[GDTR as usize..GDTR as usize + 2].copy_from_slice(&31u16.to_le_bytes());
    for (offset, desc) in [(8, 0x00cf9a000000ffffu64), (16, 0x00cf92000000ffff)] {
        b[GDT as usize + offset..GDT as usize + offset + 8].copy_from_slice(&desc.to_le_bytes());
    }
    Ok(b)
}

fn main() {
    let args: Vec<_> = env::args().collect();
    assert_eq!(args.len(), 3, "rust16-device caller|header OUTPUT");
    let bytes = match args[1].as_str() {
        "caller" => caller(),
        "header" => image(),
        _ => panic!("unknown output"),
    }
    .expect("encode x86 architecture adapter");
    fs::write(&args[2], bytes).unwrap();
}
