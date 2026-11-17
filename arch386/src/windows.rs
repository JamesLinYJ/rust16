//! Windows 386-enhanced loader ABI, INT 2F/1605h far callback (AX=0/1).
use super::*;

// The DOS monitor initialises the descriptors and the V86 return frame.
const RM_GDTR: i32 = 0x280;
const RM_IDTR: i32 = 0x286;
const PM_GDTR: i32 = 0x28c;
const PM_IDTR: i32 = 0x292;
const PM_CR0: i32 = 0x298;
const PM_CR3: i32 = 0x29c;
const CHECK_STACK: i32 = 0x2a0;
const RM_JUMP: i32 = 0x2a8;
const SMALL_GDTR: i32 = 0x2ac;
const SUSPENDED: i32 = 0x2cc;
const V86: i32 = 0x2d8;
const CLIENT_SP: i32 = V86 + 12;
const CLIENT_SS: i32 = V86 + 16;

fn table(a: &mut CodeAssembler, code: Code, at: i32) -> Result<()> {
    // Select a 32-bit descriptor base even in a 16-bit instruction stream.
    let mut ins = Instruction::with1(code, MemoryOperand::with_displ(at as u64, 2))?;
    ins.set_segment_prefix(Register::CS);
    a.add_instruction(ins)
}
pub(super) fn save_real_tables(a: &mut CodeAssembler) -> Result<()> {
    table(a, Code::Sgdt_m1632, RM_GDTR)?;
    table(a, Code::Sidt_m1632, RM_IDTR)
}

pub(super) fn entries(b: &mut [u8]) -> Result<()> {
    let mut a = CodeAssembler::new(16)?;
    a.int(0xe6)?; // After the previous 1605h interrupt handler returns.
    a.iret()?;
    place(b, 0x210, 0x220, &mut a)?;

    // IRET from the ring-0 monitor reaches this entry in its own address space.
    let mut a = CodeAssembler::new(32)?;
    a.sgdt(fword_ptr(PM_GDTR))?;
    a.sidt(fword_ptr(PM_IDTR))?;
    a.mov(eax, cr0)?;
    a.mov(dword_ptr(PM_CR0), eax)?;
    a.mov(eax, cr3)?;
    a.mov(dword_ptr(PM_CR3), eax)?;
    a.jmp(0x468u64)?;
    place(b, 0x438, 0x468, &mut a)?;
    let mut a = CodeAssembler::new(32)?;
    a.lgdt(fword_ptr(SMALL_GDTR))?;
    a.jmp_far(8, 0x780)?;
    place(b, 0x468, 0x480, &mut a)?;

    let mut a = CodeAssembler::new(32)?;
    a.mov(ax, 16)?;
    a.mov(ds, ax)?;
    a.mov(es, ax)?;
    a.mov(ss, ax)?;
    a.mov(esp, STACK_TOP)?;
    // While paging is still off, verify the real return stack against the
    // saved monitor mappings. Leave the caller in real mode on failure.
    a.cld()?;
    a.call(dword_ptr(CHECK_STACK))?;
    a.test(eax, eax)?;
    a.jnz(0x5b0u64)?;
    a.jmp(0x468u64)?;
    place(b, 0x218, 0x240, &mut a)?;
    let mut a = CodeAssembler::new(32)?;
    a.mov(eax, dword_ptr(PM_CR3))?;
    a.mov(cr3, eax)?;
    a.mov(eax, dword_ptr(PM_CR0))?;
    a.mov(cr0, eax)?;
    a.lidt(fword_ptr(PM_IDTR))?;
    a.xor(eax, eax)?;
    a.lldt(ax)?;
    a.and(byte_ptr(0x33d), 0xfd)?; // reload the monitor's TSS
    a.mov(al, 24)?;
    a.ltr(ax)?;
    a.cld()?;
    a.call(dword_ptr(0x3ac))?; // restore monitor PIC vectors
    a.mov(byte_ptr(SUSPENDED), 0)?;
    a.mov(esp, V86)?;
    a.iretd()?;
    place(b, 0x5b0, 0x600, &mut a)?;

    let mut a = CodeAssembler::new(16)?;
    let mut disable = a.create_label();
    let mut error = a.create_label();
    a.pushfd()?;
    a.cli()?;
    a.pushad()?;
    a.push(ds)?;
    a.push(es)?;
    a.push(fs)?;
    a.push(gs)?;
    a.mov(bp, sp)?;
    // IF must stay clear on both returns. Other caller flags survive.
    a.and(word_ptr(bp + 40), 0xfdfe)?;
    a.mov(bx, cs)?;
    a.mov(ds, bx)?;
    a.smsw(bx)?;
    a.test(ax, ax)?;
    a.je(disable)?;
    a.cmp(ax, 1)?;
    a.jne(error)?;
    a.test(bl, 1)?;
    a.jne(error)?;
    a.cmp(byte_ptr(SUSPENDED), 1)?;
    a.jne(error)?;
    a.mov(dword_ptr(CLIENT_SP), esp)?;
    a.mov(word_ptr(CLIENT_SS), ss)?;
    save_real_tables(&mut a)?;
    table(&mut a, Code::Lgdt_m1632, PM_GDTR)?;
    a.mov(eax, cr0)?;
    a.or(al, 1)?;
    a.mov(cr0, eax)?;
    a.jmp_far(8, 0x218)?;
    a.set_label(&mut disable)?;
    a.test(bl, 1)?;
    a.je(error)?;
    a.jmp(0x7f4u64)?;
    a.set_label(&mut error)?;
    a.jmp(0x7e0u64)?;
    place(b, 0x700, 0x780, &mut a)?;

    let mut a = CodeAssembler::new(16)?;
    a.mov(ax, 16)?;
    for segment in [ds, es, ss, fs, gs] {
        a.mov(segment, ax)?;
    }
    a.mov(eax, cr0)?;
    a.and(eax, 0x7fffffffu32)?;
    a.mov(cr0, eax)?;
    a.mov(eax, cr0)?;
    a.and(al, 0xfe)?;
    a.mov(cr0, eax)?;
    let mut jump = Instruction::with1(
        Code::Jmp_m1616,
        MemoryOperand::with_displ(RM_JUMP as u64, 2),
    )?;
    jump.set_segment_prefix(Register::CS);
    a.add_instruction(jump)?;
    place(b, 0x780, 0x7a8, &mut a)?;

    let mut a = CodeAssembler::new(16)?;
    table(&mut a, Code::Lgdt_m1632, RM_GDTR)?;
    table(&mut a, Code::Lidt_m1632, RM_IDTR)?;
    a.mov(ss, word_ptr(CLIENT_SS).cs())?;
    a.mov(esp, dword_ptr(CLIENT_SP).cs())?;
    a.jmp(0x7ccu64)?;
    place(b, 0x7a8, 0x7cc, &mut a)?;

    let mut a = CodeAssembler::new(16)?;
    a.pop(gs)?;
    a.pop(fs)?;
    a.pop(es)?;
    a.pop(ds)?;
    a.popad()?;
    a.popfd()?;
    a.retf()?;
    place(b, 0x7cc, 0x7e0, &mut a)?;
    let mut a = CodeAssembler::new(16)?;
    a.or(word_ptr(bp + 40), 1)?;
    a.jmp(0x7ccu64)?;
    place(b, 0x7e0, 0x7f4, &mut a)?;
    let mut a = CodeAssembler::new(16)?;
    a.int(0xe6)?; // successful handoff resumes at the common far return in real mode
    a.jmp(0x7e0u64)?;
    place(b, 0x7f4, 0x800, &mut a)
}

/// A standalone DOS COM client of the DDK interface. Uses only the public
/// broadcast and its returned far address, with no knowledge of the monitor.
pub(super) fn probe() -> Result<Vec<u8>> {
    let mut a = CodeAssembler::new(16)?;
    let mut fail = a.create_label();
    let mut report = a.create_label();
    const ENTRY: i32 = 0x500;
    const SP0: i32 = 0x504;
    const SS0: i32 = 0x506;
    const IDT0: i32 = 0x508;
    const IDT1: i32 = 0x50e;
    const GDT0: i32 = 0x514;
    const GDT1: i32 = 0x51a;
    const ORIGINAL_SP: i32 = 0x520;
    const ORIGINAL_SS: i32 = 0x522;
    a.pushf()?;
    a.mov(word_ptr(ORIGINAL_SP).cs(), sp)?;
    a.mov(word_ptr(ORIGINAL_SS).cs(), ss)?;
    a.xor(bx, bx)?;
    a.mov(cx, bx)?;
    a.mov(dx, bx)?;
    a.mov(si, bx)?;
    a.mov(es, bx)?;
    a.mov(ds, bx)?;
    a.mov(di, 0x30a)?;
    a.mov(ax, 0x1605)?;
    a.int(0x2f)?;
    a.test(cx, cx)?;
    a.jne(fail)?;
    a.mov(word_ptr(ENTRY).cs(), si)?;
    a.mov(word_ptr(ENTRY + 2).cs(), ds)?;
    a.push(cs)?;
    a.pop(ds)?;
    a.cmp(dword_ptr(ENTRY), 0)?;
    a.je(fail)?;
    // A 1605h broadcast with an existing owner must retain its far pointer.
    a.mov(si, 0x1234)?;
    a.xor(cx, cx)?;
    a.xor(dx, dx)?;
    a.mov(ax, 0x1605)?;
    a.int(0x2f)?;
    a.test(cx, cx)?;
    a.je(fail)?;
    a.cmp(si, 0x1234)?;
    a.jne(fail)?;
    a.mov(dx, ds)?;
    a.mov(bx, cs)?;
    a.cmp(dx, bx)?;
    a.jne(fail)?;
    a.cli()?;
    a.mov(word_ptr(SP0), sp)?;
    a.mov(word_ptr(SS0), ss)?;
    table(&mut a, Code::Sidt_m1632, IDT0)?;
    table(&mut a, Code::Sgdt_m1632, GDT0)?;
    for (step, mode) in [0, 1, 0, 1].into_iter().enumerate() {
        if step == 3 {
            // Windows can shut down on a different stack than it started on.
            a.mov(dx, ss)?;
            a.inc(dx)?;
            a.mov(bx, sp)?;
            a.sub(bx, 0x100)?;
            a.mov(ss, dx)?;
            a.mov(sp, bx)?;
            a.mov(word_ptr(SP0), sp)?;
            a.mov(word_ptr(SS0), ss)?;
        }
        a.mov(ebx, 0x87654321u32)?;
        a.mov(ebp, 0x13572468)?;
        a.mov(ax, mode)?;
        a.add_instruction(Instruction::with1(
            Code::Call_m1616,
            MemoryOperand::with_displ(ENTRY as u64, 2),
        )?)?;
        a.pushf()?;
        a.pop(dx)?;
        a.test(dx, 0x201)?;
        a.jne(fail)?;
        a.cmp(ebx, 0x87654321u32)?;
        a.jne(fail)?;
        a.cmp(ebp, 0x13572468)?;
        a.jne(fail)?;
        a.mov(dx, ss)?;
        a.cmp(dx, word_ptr(SS0))?;
        a.jne(fail)?;
        a.cmp(sp, word_ptr(SP0))?;
        a.jne(fail)?;
        a.smsw(dx)?;
        a.and(dx, 1)?;
        a.cmp(dx, mode)?;
        a.jne(fail)?;
        table(&mut a, Code::Sidt_m1632, IDT1)?;
        table(&mut a, Code::Sgdt_m1632, GDT1)?;
        if mode == 0 {
            a.mov(eax, cr0)?;
            a.test(eax, 0x80000001u32)?;
            a.jne(fail)?;
            a.cmp(dword_ptr(IDT1 + 2), 0)?; // BIOS IVT on this cold boot
            a.jne(fail)?;
        } else {
            for (before, after) in [(IDT0, IDT1), (GDT0, GDT1)] {
                a.mov(ax, word_ptr(before))?;
                a.cmp(ax, word_ptr(after))?;
                a.jne(fail)?;
                a.mov(eax, dword_ptr(before + 2))?;
                a.cmp(eax, dword_ptr(after + 2))?;
                a.jne(fail)?;
            }
        }
    }
    a.mov(dx, 0x530)?;
    a.jmp(report)?;
    a.set_label(&mut fail)?;
    a.mov(dx, 0x580)?;
    a.set_label(&mut report)?;
    a.mov(ss, word_ptr(ORIGINAL_SS).cs())?;
    a.mov(sp, word_ptr(ORIGINAL_SP).cs())?;
    a.push(cs)?;
    a.pop(ds)?;
    a.popf()?;
    a.mov(ah, 9)?;
    a.int(0x21)?;
    a.mov(ax, 0x4c00)?;
    a.int(0x21)?;
    let mut bytes = a.assemble(0x100)?;
    assert!(bytes.len() < ENTRY as usize - 0x100);
    bytes.resize(0x430, 0);
    bytes.extend(b"WINMODE PASS: V86 / real / V86 twice, far stack, flags and tables\r\n$");
    bytes.resize(0x480, 0);
    bytes.extend(b"WINMODE FAIL\r\n$");
    Ok(bytes)
}
