//! CPU and calling-convention adapters for segmented DOS device images.
//! The monitor itself is an independently linked i386 Rust image at offset 0x800.
use iced_x86::{Code, IcedError, Instruction, MemoryOperand, Register, code_asm::*};
use std::{env, fs};
mod windows;
mod console;
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
    console::adapter(&mut s)?;
    Ok(s.finish().expect("encode DOS device-call OMF"))
}

// Portable DOS client ABI. The near packet lives in the Rust16 stack/data
// segment. Its words hold AX BX CX DX SI DI ES, XMS far pointer, and selector.
fn client() -> Result<Vec<u8>> {
    let mut s = rust16::machine::Segment::default();
    let mut a = CodeAssembler::new(16)?;
    a.mov(ax, ss)?;
    a.retf()?;
    s.public("dos_data_segment");
    s.emit(&a.assemble(0)?);
    let mut a = CodeAssembler::new(16)?;
    a.push(bp)?;
    a.mov(bp, sp)?;
    a.pushf()?;
    a.pusha()?;
    a.push(es)?;
    a.push(ds)?;
    a.mov(bp, word_ptr(bp + 6))?;
    a.mov(ax, word_ptr(bp))?;
    a.mov(bx, word_ptr(bp + 2))?;
    a.mov(cx, word_ptr(bp + 4))?;
    a.mov(dx, word_ptr(bp + 6))?;
    a.mov(si, word_ptr(bp + 8))?;
    a.mov(di, word_ptr(bp + 10))?;
    a.mov(es, word_ptr(bp + 12))?;
    let mut vcpi = a.create_label();
    let mut ems = a.create_label();
    let mut multiplex = a.create_label();
    let mut dos = a.create_label();
    let mut done = a.create_label();
    a.cmp(word_ptr(bp + 18), 4)?;
    a.je(vcpi)?;
    a.cmp(word_ptr(bp + 18), 0)?;
    a.je(ems)?;
    a.cmp(word_ptr(bp + 18), 1)?;
    a.je(multiplex)?;
    a.cmp(word_ptr(bp + 18), 3)?;
    a.je(dos)?;
    a.add_instruction(Instruction::with1(
        Code::Call_m1616,
        MemoryOperand::with_base_displ(Register::BP, 14),
    )?)?;
    a.jmp(done)?;
    a.set_label(&mut vcpi)?;
    a.movzx(edx, word_ptr(bp + 22))?;
    a.shl(edx, 16)?;
    a.mov(dx, word_ptr(bp + 6))?;
    a.int(0x67)?;
    a.push(edx)?;
    a.shr(edx, 16)?;
    a.mov(word_ptr(bp + 22), dx)?;
    a.pop(edx)?;
    a.jmp(done)?;
    a.set_label(&mut ems)?;
    a.int(0x67)?;
    a.jmp(done)?;
    a.set_label(&mut multiplex)?;
    a.int(0x2f)?;
    a.jmp(done)?;
    a.set_label(&mut dos)?;
    a.int(0x21)?;
    a.set_label(&mut done)?;
    a.mov(word_ptr(bp), ax)?;
    a.mov(word_ptr(bp + 2), bx)?;
    a.mov(word_ptr(bp + 4), cx)?;
    a.mov(word_ptr(bp + 6), dx)?;
    a.mov(word_ptr(bp + 10), di)?;
    a.mov(word_ptr(bp + 12), es)?;
    a.pushf()?;
    a.pop(ax)?;
    a.mov(word_ptr(bp + 20), ax)?;
    a.pop(ds)?;
    a.pop(es)?;
    a.popa()?;
    a.popf()?;
    a.pop(bp)?;
    a.retf()?;
    s.public("dos_memory_call");
    s.emit(&a.assemble(0)?);
    Ok(s.finish().expect("encode DOS memory client OMF"))
}
fn upper_caller() -> Result<Vec<u8>> {
    let mut s = rust16::machine::Segment::default();
    let mut a = CodeAssembler::new(16)?;
    a.push(bp)?;
    a.mov(bp, sp)?;
    a.push(es)?;
    a.push(bx)?;
    a.push(cx)?;
    a.push(si)?;
    a.push(di)?;
    a.xor(ax, ax)?;
    a.mov(es, ax)?;
    a.xor(dx, dx)?;
    a.les(di, dword_ptr(0x2f * 4).es())?;
    let mut done = a.create_label();
    let mut absent = a.create_label();
    a.mov(ax, es)?;
    a.or(ax, di)?;
    a.je(absent)?;
    a.mov(cx, word_ptr(bp + 12))?;
    a.mov(bx, word_ptr(bp + 10))?;
    a.mov(dx, word_ptr(bp + 8))?;
    a.mov(si, word_ptr(bp + 6))?;
    a.mov(ax, 0xd7a0)?;
    a.mov(di, 0x5253)?;
    a.int(0x2f)?;
    // Later residents may interpose on INT 2Fh and chain to our handler.
    a.cmp(di, 0x5352)?;
    a.je(done)?;
    a.set_label(&mut absent)?;
    a.xor(ax, ax)?;
    a.xor(dx, dx)?;
    a.set_label(&mut done)?;
    a.pop(di)?;
    a.pop(si)?;
    a.pop(cx)?;
    a.pop(bx)?;
    a.pop(es)?;
    a.pop(bp)?;
    a.retf()?;
    s.public("dos_upper_request");
    s.emit(&a.assemble(0)?);
    Ok(s.finish().expect("encode DOS upper arena ABI"))
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
const STACK_TOP: u32 = 0xe000;

fn image() -> Result<Vec<u8>> {
    let mut b = vec![0u8; 0x800];
    b[..4].copy_from_slice(&u32::MAX.to_le_bytes());
    b[4..6].copy_from_slice(&0xc000u16.to_le_bytes()); // character device with IOCTL
    b[6..8].copy_from_slice(&(STRATEGY as u16).to_le_bytes());
    b[8..10].copy_from_slice(&(INTERRUPT as u16).to_le_bytes());
    b[10..18].copy_from_slice(b"EMMXXXX0");
    b[IRET_ENTRY + 10..IRET_ENTRY + 18].copy_from_slice(b"EMMXXXX0");
    let mut a = CodeAssembler::new(16)?;
    a.int(0xe6)?;
    a.iret()?;
    place(&mut b, IRET_ENTRY, IRET_ENTRY + 10, &mut a)?;
    let mut command = CodeAssembler::new(16)?;
    command.int(0xe6)?;
    command.ret()?;
    place(&mut b, 0x24, 0x2a, &mut command)?;
    let mut x = CodeAssembler::new(16)?;
    x.int(0xe6)?;
    x.retf()?;
    place(&mut b, 0x32, 0x36, &mut x)?;
    // XMS 3.0 Appendix A. Windows patches the five-byte hook with a FAR JMP
    // and chains to the entry immediately following it.
    let mut hook = CodeAssembler::new(16)?;
    let mut dispatch = hook.create_label();
    hook.jmp(dispatch)?;
    hook.nop()?;
    hook.nop()?;
    hook.nop()?;
    hook.set_label(&mut dispatch)?;
    hook.jmp(0x32u64)?;
    place(&mut b, 0x12, IRET_ENTRY, &mut hook)?;
    let mut x = CodeAssembler::new(16)?;
    x.int(0xe6)?;
    x.iret()?;
    place(&mut b, 0x36, 0x3a, &mut x)?;
    let mut x = CodeAssembler::new(16)?;
    x.int(0xe6)?;
    x.iret()?;
    place(&mut b, 0x3a, 0x40, &mut x)?;
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
    // While the VMM owns the processor it may query output status in real mode.
    let mut monitor = a.create_label();
    a.smsw(ax)?;
    a.test(al, 1)?;
    a.jne(monitor)?;
    a.mov(word_ptr(bx + 3).es(), 0x8103)?;
    a.cmp(byte_ptr(bx + 2).es(), 10)?;
    a.jne(done)?;
    a.mov(word_ptr(bx + 3).es(), 0x100)?;
    a.jmp(done)?;
    a.set_label(&mut monitor)?;
    a.call(0x24u64)?;
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
    a.add(ax, ((STACK_TOP + 0x1000) / 16) as i32)?;
    a.cmp(ax, word_ptr(bx + 16).es())?;
    a.ja(skip)?;
    a.mov(ah, 0x88)?;
    a.int(0x15)?;
    a.jb(skip)?;
    a.cmp(ax, 128)?;
    a.jb(skip)?;
    a.mov(word_ptr(EXT_KIB), ax)?;
    a.call(0x600u64)?;
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
    a.add(ax, ((STACK_TOP + 0x1000) / 16) as i32)?;
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
    a.call(0x800u64)?;
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
    vcpi_entries(&mut b)?;
    physical_caps(&mut b)?;
    windows::entries(&mut b)?;
    Ok(b)
}

// Called in real mode, after both 386 and DOS load-boundary checks. The final
// resident page is E820 scratch until the monitor publishes the XMS table.
fn physical_caps(b: &mut [u8]) -> Result<()> {
    let mut a = CodeAssembler::new(16)?;
    let mut done = a.create_label();
    let mut fallback = a.create_label();
    let mut map = a.create_label();
    let mut next = a.create_label();
    let mut complete = a.create_label();
    a.pushad()?;
    windows::save_real_tables(&mut a)?;
    a.push(es)?;
    a.pushfd()?;
    a.pop(eax)?;
    a.mov(ecx, eax)?;
    a.xor(eax, 1 << 21)?;
    a.push(eax)?;
    a.popfd()?;
    a.pushfd()?;
    a.pop(eax)?;
    a.push(ecx)?;
    a.popfd()?;
    a.xor(eax, ecx)?;
    a.test(eax, 1 << 21)?;
    a.je(done)?;
    a.xor(eax, eax)?;
    a.cpuid()?;
    a.test(eax, eax)?;
    a.je(done)?;
    a.mov(eax, 1)?;
    a.cpuid()?;
    a.and(edx, 0x48)?; // PAE and PSE
    a.cmp(edx, 0x48)?;
    a.jne(done)?;
    a.mov(eax, 0x80000000u32)?;
    a.cpuid()?;
    a.cmp(eax, 0x80000008u32)?;
    a.jb(fallback)?;
    a.mov(eax, 0x80000008u32)?;
    a.cpuid()?;
    a.jmp(map)?;
    a.set_label(&mut fallback)?;
    a.mov(al, 36)?;
    a.set_label(&mut map)?;
    a.mov(byte_ptr(0x3c0), al)?;
    a.push(cs)?;
    a.pop(es)?;
    a.mov(di, STACK_TOP as u16 as i32)?;
    a.xor(ebx, ebx)?;
    a.set_label(&mut next)?;
    a.mov(dword_ptr(di + 20).es(), 1)?;
    a.mov(ecx, 24)?;
    a.mov(edx, 0x534d4150)?;
    a.mov(eax, 0xe820)?;
    a.push(ds)?;
    a.push(es)?;
    a.push(di)?;
    a.int(0x15)?;
    a.pop(di)?;
    a.pop(es)?;
    a.pop(ds)?;
    a.jb(done)?;
    a.cmp(eax, 0x534d4150)?;
    a.jne(done)?;
    let mut valid = a.create_label();
    a.cmp(ecx, 24)?;
    a.jae(valid)?;
    a.cmp(ecx, 20)?;
    a.jne(done)?;
    a.mov(dword_ptr(di + 20).es(), 1)?;
    a.set_label(&mut valid)?;
    a.add(di, 24)?;
    a.test(ebx, ebx)?;
    a.je(complete)?;
    a.cmp(di, STACK_TOP as i32 + 64 * 24)?;
    a.jb(next)?;
    a.jmp(done)?; // truncated map
    a.set_label(&mut complete)?;
    a.sub(di, STACK_TOP as i32)?;
    a.mov(word_ptr(0x3c2), di)?;
    a.set_label(&mut done)?;
    a.pop(es)?;
    a.popad()?;
    a.ret()?;
    place(b, 0x600, 0x700, &mut a)
}

// The first host descriptor supplied by DE01h addresses this header in both
// page directories. The adjacent descriptor is its writable data alias.
fn vcpi_entries(b: &mut [u8]) -> Result<()> {
    let mut a = CodeAssembler::new(32)?;
    // Rust validated and copied the client's pseudo-descriptors and selectors.
    // The caller owns EBX and uses it to establish its protected-mode stack.
    a.cli()?;
    a.mov(eax, dword_ptr(0x380).cs())?;
    a.mov(cr3, eax)?;
    a.lgdt(fword_ptr(0x384).cs())?;
    a.lidt(fword_ptr(0x38a).cs())?;
    a.lldt(word_ptr(0x390).cs())?;
    a.ltr(word_ptr(0x392).cs())?;
    a.clts()?;
    let mut jump = Instruction::with1(Code::Jmp_m1632, MemoryOperand::with_displ(0x394, 4))?;
    jump.set_segment_prefix(Register::CS);
    a.add_instruction(jump)?;
    place(b, 0x400, 0x480, &mut a)?;
    let mut a = CodeAssembler::new(32)?;
    let mut service = a.create_label();
    let mut invalid = a.create_label();
    a.cmp(ax, 0xde0c)?;
    a.jne(service)?;
    // Copy the client's IRETD frame before its linear stack disappears.
    a.cli()?;
    a.mov(ax, cs)?;
    a.add(ax, 8)?;
    a.mov(ds, ax)?;
    a.mov(edx, esp)?;
    a.add(edx, 8)?;
    for i in 0..9 {
        a.mov(eax, dword_ptr(edx + 4 * i).ss())?;
        a.mov(dword_ptr(0x350 + i * 4), eax)?;
    }
    // Carry, direction, interrupt and arithmetic flags belong to the returning client.
    a.and(dword_ptr(0x358), 0xfd5)?;
    a.or(dword_ptr(0x358), 0x23002)?;
    a.mov(eax, dword_ptr(0x39c))?;
    a.mov(cr3, eax)?;
    a.lgdt(fword_ptr(GDTR))?;
    a.lidt(fword_ptr(0x3a0))?;
    a.xor(eax, eax)?;
    a.lldt(ax)?;
    a.and(byte_ptr(0x33d), 0xfd)?;
    a.mov(al, 24)?;
    a.ltr(ax)?;
    a.clts()?;
    a.mov(ax, 16)?;
    a.mov(ss, ax)?;
    a.mov(ds, ax)?;
    a.mov(es, ax)?;
    a.mov(esp, STACK_TOP)?;
    a.cld()?;
    a.call(dword_ptr(0x3ac))?;
    a.mov(esp, 0x350)?;
    a.iretd()?;
    a.set_label(&mut service)?;
    a.cmp(ax, 0xde03)?;
    a.jb(invalid)?;
    a.cmp(ax, 0xde05)?;
    a.ja(invalid)?;
    a.pushfd()?;
    a.cli()?;
    a.cld()?;
    a.pushad()?;
    a.push(ds)?;
    a.push(es)?;
    a.mov(ebx, esp)?;
    a.xor(ecx, ecx)?;
    a.mov(cx, ss)?;
    a.mov(si, cs)?;
    a.add(si, 8)?;
    a.mov(ds, si)?;
    a.mov(es, si)?;
    a.mov(edi, cr3)?;
    a.mov(ss, si)?;
    a.mov(esp, STACK_TOP)?;
    a.mov(esi, dword_ptr(0x39c))?;
    a.mov(cr3, esi)?;
    a.push(ecx)?;
    a.push(ebx)?;
    a.push(edi)?;
    a.push(edx)?;
    a.push(eax)?;
    a.call(dword_ptr(0x3a8))?;
    a.add(esp, 8)?;
    a.pop(edi)?;
    a.pop(ebx)?;
    a.pop(ecx)?;
    a.mov(cr3, edi)?;
    a.mov(ss, cx)?;
    a.mov(esp, ebx)?;
    a.mov(dword_ptr(esp + 36), eax)?;
    a.mov(dword_ptr(esp + 28), edx)?;
    a.pop(es)?;
    a.pop(ds)?;
    a.popad()?;
    a.popfd()?;
    a.retf()?;
    a.set_label(&mut invalid)?;
    a.mov(ah, 0x84)?;
    a.retf()?;
    place(b, 0x480, 0x600, &mut a)
}

fn main() {
    let args: Vec<_> = env::args().collect();
    assert_eq!(
        args.len(),
        3,
        "rust16-device caller|header|client|upper|winprobe OUTPUT"
    );
    let bytes = match args[1].as_str() {
        "caller" => caller(),
        "client" => client(),
        "upper" => upper_caller(),
        "header" => image(),
        "winprobe" => windows::probe(),
        _ => panic!("unknown output"),
    }
    .expect("encode x86 architecture adapter");
    fs::write(&args[2], bytes).unwrap();
}
