//! Real-mode firmware ABI. The kernel and second-stage loader call these far functions.
//! On entry DL holds the BIOS disk. DS = SS names the kernel's data and stack segment.
use crate::machine::Segment;

// Immediately below the first application MCB at 3fff:0000. The low 4 KiB
// holds the near literal pool, the next 2 KiB holds AUTOEXEC, and the stack
// grows down from 3fff:0000 without occupying the conventional process arena.
const KERNEL_STACK: u16 = 0x3900;

pub(crate) fn function(s: &mut Segment, name: &str, body: &[u8]) {
    s.public(name);
    s.emit(&[0x55, 0x89, 0xe5]); // push bp; mov bp,sp
    s.emit(body);
    s.emit(&[0x5d, 0xcb]);
}

pub fn object(kernel: bool) -> Result<Vec<u8>, String> {
    let mut s = Segment::default();
    s.public("_start");
    s.emit(&[0xfa, 0xb8]);
    s.word(KERNEL_STACK);
    s.emit(&[
        0x8e, 0xd0, 0xbc, 0xe0, 0x6f, 0x8e, 0xd8, 0x88, 0x16, 0, 0, 0xfc, 0xfb,
    ]);
    s.emit(&[0xc7, 0x06, 2, 0, 0, 0]); // No suspended EXEC (also after a warm boot).
    s.emit(&[0x31, 0xd2, 0xb8, 0xe3, 0, 0xcd, 0x14]); // COM1, 9600 8N1
    crate::literals::init(&mut s);
    s.far_call("main16");
    s.public("rust16_abort");
    s.label("stop");
    s.emit(&[0xfb, 0xf4]);
    s.relative(0xe9, "stop");

    // Real-mode FLAGS: pre-286 high bits are set, 286 high bits cannot be set,
    // and 386 real mode permits toggling IOPL. Restore the original FLAGS.
    s.public("bios_cpu");
    s.emit(&[
        0x9c, 0x59, 0x89, 0xc8, 0x25, 0xff, 0x0f, 0x50, 0x9d, 0x9c, 0x58, 0xa9, 0, 0xf0,
    ]);
    s.branch(0x75, "early");
    s.emit(&[
        0x89, 0xc8, 0x0d, 0, 0x70, 0x50, 0x9d, 0x9c, 0x58, 0xa9, 0, 0x70,
    ]);
    s.branch(0x74, "at");
    s.emit(&[0xb8]);
    s.word(386);
    s.relative(0xe9, "restore");
    s.label("at");
    s.emit(&[0xb8]);
    s.word(286);
    s.relative(0xe9, "restore");
    s.label("early");
    s.emit(&[0xb8, 86, 0]);
    s.label("restore");
    s.emit(&[0x51, 0x9d, 0xcb]);

    function(
        &mut s,
        "bios_read",
        &[
            0x8b, 0x46, 6, 0x8e, 0xc0, 0x31, 0xdb, // destination paragraph, offset zero
            0x8b, 0x4e, 10, 0x8a, 0x76, 8, 0x8a, 0x16, 0, 0, // CHS, head, saved drive
            0xb8, 1, 2, 0xcd, 0x13, 0x19, 0xc0, // 0 success / ffff on carry
        ],
    );
    function(
        &mut s,
        "bios_reset",
        &[0x31, 0xc0, 0x8a, 0x16, 0, 0, 0xcd, 0x13],
    );
    memory(&mut s);
    s.public("bios_char");
    s.emit(&[0x55, 0x89, 0xe5, 0x8b, 0x46, 6]);
    s.relative(0xe8, "tty");
    s.emit(&[0x5d, 0xcb]);
    s.public("bios_text");
    s.emit(&[0x55, 0x89, 0xe5, 0x8b, 0x76, 6, 0x8b, 0x4e, 8]);
    s.label("text");
    s.emit(&[0x09, 0xc9]);
    s.branch(0x74, "text-end");
    s.emit(&[0xac]);
    s.relative(0xe8, "tty");
    s.emit(&[0x49]);
    s.relative(0xe9, "text");
    s.label("text-end");
    s.emit(&[0x5d, 0xcb]);
    function(&mut s, "bios_key", &[0x31, 0xc0, 0xcd, 0x16, 0x30, 0xe4]);
    s.public("bios_chain");
    s.emit(&[0x8a, 0x16, 0, 0, 0xea, 0, 0, 0, 0x10]); // DL retained at the new entry
    s.public("bios_stop");
    s.relative(0xe9, "stop");

    // Run a COM in its own 64 KiB segment. Termination restores the suspended
    // kernel stack through the installed INT 20h / INT 21h gates.
    s.public("bios_run");
    s.emit(&[0x55, 0x89, 0xe5]);
    // Each suspended EXEC keeps its own kernel-stack pointer, volume and caller frame.
    for field in [2, 4, 6, 8] {
        s.emit(&[0xff, 0x36, field, 0]);
    }
    s.emit(&[0x89, 0x26, 2, 0, 0x31, 0xc0, 0x8e, 0xc0, 0xfa]);
    s.emit(&[0x8b, 0x46, 16, 0xa3, 4, 0]);
    // An EXEC inherits the live IVT, including hooks installed by its parent.
    s.emit(&[0x83, 0x7e, 0xfe, 0]);
    s.branch(0x75, "run-ready");
    for vector in [0x20u16, 0x21] {
        // position-independent offset from a near CALL and a relative ADD
        s.relative(0xe8, &format!("vector-{vector}"));
        s.label(&format!("vector-{vector}"));
        s.emit(&[0x58, 0x05]);

        s.word(0);
        s.emit(&[0x26, 0xa3]);
        s.word(vector * 4);
        s.emit(&[0x26, 0x8c, 0x0e]);
        s.word(vector * 4 + 2);
        // Patch after the handler offset is known.
    }
    s.label("run-ready");
    s.emit(&[
        0x8b, 0x56, 12, 0x8b, 0x4e, 10, 0x8b, 0x76, 8, 0x8b, 0x7e, 6, 0x8b, 0x46, 14, 0x8e, 0xd8,
        0x8e, 0xc0, 0x8e, 0xd6, 0x89, 0xfc, 0x52, 0x51, 0x31, 0xc0, 0xfb, 0xfc, 0xcb,
    ]);
    let int21 = s.bytes.len();
    // INT 21h AH=02h,09h and 4Ch. IRET preserves caller FLAGS.
    s.emit(&[0x80, 0xfc, 0x4c]);
    s.branch(0x74, "terminate");
    s.emit(&[0x84, 0xe4]);
    s.branch(0x74, "exit-zero");
    if kernel {
        // Snapshot the caller, then run the DOS service on the suspended kernel
        // stack. Rust modifies the saved registers and CF in the interrupt frame.
        s.emit(&[
            0x1e, 0x06, 0x55, 0x57, 0x56, 0x52, 0x51, 0x53, 0x50, 0x8c, 0xd1, 0x89, 0xe3, 0xb8,
        ]);
        s.word(KERNEL_STACK);
        s.emit(&[
            0x8e, 0xd8, 0x89, 0x0e, 6, 0, 0x89, 0x1e, 8, 0, 0xfa, 0x8e, 0xd0, 0x8b, 0x26, 2, 0,
            0xff, 0x36, 4, 0, 0x51, 0x53, 0xfc, 0xfb,
        ]);
        s.far_call("dos16_service");
        s.emit(&[
            0xfa, 0x8b, 0x0e, 6, 0, 0x8b, 0x1e, 8, 0, 0x8e, 0xd1, 0x89, 0xdc, 0x58, 0x5b, 0x59,
            0x5a, 0x5e, 0x5f, 0x5d, 0x07, 0x1f, 0xcf,
        ]);
    } else {
        s.emit(&[0x55, 0x89, 0xe5, 0x83, 0x4e, 6, 1, 0x5d, 0xb8, 1, 0, 0xcf]);
    }
    s.label("tty");
    s.emit(&[
        0x50, 0x53, 0x52, 0x51, 0x50, 0xba, 0xfd, 3, 0xb9, 0xff, 0xff,
    ]);
    s.label("uart-ready");
    s.emit(&[0xec, 0xa8, 0x40]);
    s.branch(0x75, "uart-send");
    s.emit(&[0x49]);
    s.branch(0x75, "uart-ready");
    s.emit(&[0x58]);
    s.relative(0xe9, "vga");
    s.label("uart-send");
    s.emit(&[0x58, 0xba, 0xf8, 3, 0xee]);
    s.label("vga");
    s.emit(&[
        0xb4, 0x0e, 0xbb, 7, 0, 0xcd, 0x10, 0x59, 0x5a, 0x5b, 0x58, 0xc3,
    ]);
    s.label("exit-zero");
    let exit_zero = s.bytes.len();
    s.emit(&[0x31, 0xc0]); // INT 20h and INT 21h/AH=00h have no status argument.
    s.label("terminate");
    s.emit(&[0xfa, 0xba]);
    s.word(KERNEL_STACK);
    s.emit(&[0x8e, 0xda, 0xa2, 10, 0, 0x8e, 0xd2, 0x8b, 0x26, 2, 0]);
    for field in [8, 6, 4, 2] {
        s.emit(&[0x8f, 0x06, field, 0]);
    }
    s.emit(&[0xfc, 0xfb, 0x5d, 0xcb]);
    for (vector, handler) in [(0x20u16, exit_zero), (0x21, int21)] {
        let from = s.label_offset(&format!("vector-{vector}"));
        let imm = from + 2;
        s.bytes[imm..imm + 2].copy_from_slice(&((handler - from) as u16).to_le_bytes());
    }
    if !kernel {
        s.public("bind_resident_io");
        s.emit(&[0xe8, 0, 0, 0x5b, 0xb8, 0xa0, 0x37, 0x8e, 0xc0]);
        let here = s.bytes.len() - 6;
        s.emit(&[
            0x2e, 0x8b, 0x47, 0, 0x26, 0xa3, 0, 0, 0x2e, 0x8b, 0x47, 0, 0x26, 0xa3, 2, 0, 0xcb,
        ]);
        let field = s.bytes.len() + 1;
        s.bytes[here + 9] = (field - here) as u8;
        s.bytes[here + 17] = (field + 2 - here) as u8;
        s.far_call("io_dispatch");
    }
    s.public("resident_io");
    s.emit(&[0xb8, 0xa0, 0x37, 0x8e, 0xc0, 0x26, 0xff, 0x2e, 0, 0]);
    if kernel {
        crate::firmware_mux::emit(&mut s, KERNEL_STACK);
    }
    crate::real_io::emit(&mut s);
    crate::literals::emit(&mut s);
    s.finish()
}

/// The FAT BPB occupies bytes 3..62. Load the Rust second stage from the
/// reserved sectors on the first track, with its MZ relocations already applied.
pub fn sector(stage_sectors: u8) -> Result<Vec<u8>, String> {
    if !(1..=17).contains(&stage_sectors) {
        return Err("loader exceeds first track".into());
    }
    let mut bytes = vec![0u8; 512];
    bytes[..3].copy_from_slice(&[0xeb, 60, 0x90]);
    let mut code = vec![
        0xfa,
        0x31,
        0xc0,
        0x8e,
        0xd0,
        0xbc,
        0,
        0x7c,
        0x8e,
        0xd8,
        0xfb,
        0xfc,
        0xbe,
        3,
        0,
        0x52,
        0xb8,
        0,
        8,
        0x8e,
        0xc0,
        0x31,
        0xdb,
        0xb9,
        2,
        0,
        0x30,
        0xf6,
        0xb8,
        stage_sectors,
        2,
        0xcd,
        0x13,
        0x5a,
        0x73,
        11,
        0x52,
        0x31,
        0xc0,
        0xcd,
        0x13,
        0x5a,
        0x4e,
        0x75,
        0xe2,
        0xcd,
        0x18,
        0xea,
        0,
        0,
        0,
        8,
    ];
    bytes[62..62 + code.len()].swap_with_slice(&mut code);
    bytes[510..].copy_from_slice(&[0x55, 0xaa]);
    Ok(bytes)
}

fn memory(s: &mut Segment) {
    function(
        s,
        "mem_byte",
        &[0x8e, 0x46, 8, 0x8b, 0x5e, 6, 0x31, 0xc0, 0x26, 0x8a, 7],
    );
    function(
        s,
        "mem_store",
        &[0x8e, 0x46, 10, 0x8b, 0x5e, 8, 0x8b, 0x46, 6, 0x26, 0x88, 7],
    );
}
pub fn utilities() -> Result<Vec<u8>, String> {
    let mut s = Segment::default();
    memory(&mut s);
    crate::real_io::emit(&mut s);
    s.finish()
}
