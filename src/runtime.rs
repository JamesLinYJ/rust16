//! DOS 2+ services, sharing a stack/data segment. Far-call arguments are
//! pushed left to right. BP, SP, SS and DS survive a call. AX holds its result.
use crate::machine::Segment;
pub fn object() -> Result<Vec<u8>, String> {
    let mut s = Segment::default();
    s.public("_start");
    // Save the PSP, switch DS and ES to the stack, and clear the bank and argv tables.
    s.emit(&[
        0xfc, 0x1e, 0x16, 0x1f, 0x16, 0x07, 0x31, 0xff, 0x31, 0xc0, 0xb9, 0xc0, 0, 0xf3, 0xab, 0x07,
    ]);
    // Copy and split the counted PSP command tail. argv[1] starts at DS:0082.
    s.emit(&[
        0x31, 0xc9, 0x26, 0x8a, 0x0e, 0x80, 0, 0xbe, 0x81, 0, 0xbf, 0, 1, 0xbb, 0x80, 0, 0x31, 0xd2,
    ]);
    s.label("scan");
    s.emit(&[0x09, 0xc9]);
    s.branch(0x74, "run");
    s.emit(&[0x26, 0xac, 0x49, 0x3c, 0x20]);
    s.branch(0x76, "space");
    s.emit(&[0x09, 0xd2]);
    s.branch(0x75, "copy");
    s.emit(&[0x83, 0xc3, 2, 0x81, 0xfb, 0xfe, 0]);
    s.branch(0x77, "bad_arg");
    s.emit(&[0x89, 0x3f, 0x42]);
    s.label("copy");
    s.emit(&[0x88, 0x05, 0x47]);
    s.relative(0xe9, "scan");
    s.label("space");
    s.emit(&[0x31, 0xd2, 0xc6, 0x05, 0, 0x47]);
    s.relative(0xe9, "scan");
    s.label("run");
    crate::literals::init(&mut s);
    s.far_call("main16");
    s.emit(&[0xb4, 0x4c, 0xcd, 0x21]);
    s.label("bad_arg");
    s.emit(&[0xb8, 10, 0x4c, 0xcd, 0x21]);
    s.label("bad_io");
    s.emit(&[0xb8, 11, 0x4c, 0xcd, 0x21]);
    s.label("bad_bank");
    s.emit(&[0xb8, 12, 0x4c, 0xcd, 0x21]);
    // AX = bank ID. ES = allocated segment. DX = largest legal byte offset.
    s.label("bank");
    s.emit(&[0x83, 0xf8, 11]);
    s.branch(0x73, "bad_bank");
    s.emit(&[
        0x89, 0xc6, 0xd1, 0xe6, 0xbb, 0, 4, 0xba, 0xff, 0x3f, 0x83, 0xf8, 1,
    ]);
    s.branch(0x76, "large");
    s.emit(&[0x83, 0xf8, 6]);
    s.branch(0x74, "large");
    s.emit(&[0x83, 0xf8, 10]);
    s.branch(0x75, "lookup");
    s.label("large");
    s.emit(&[0xbb, 0, 0x10, 0xba, 0xff, 0xff]);
    s.label("lookup");
    s.emit(&[0x8b, 0x04, 0x09, 0xc0]);
    s.branch(0x75, "have");
    s.emit(&[0xb4, 0x48, 0xcd, 0x21]);
    s.branch(0x72, "bad_bank");
    s.emit(&[
        0x89, 0x04, 0x8e, 0xc0, 0x89, 0xd9, 0xd1, 0xe1, 0xd1, 0xe1, 0xd1, 0xe1, 0x31, 0xc0, 0x31,
        0xff, 0xf3, 0xab, 0xc3,
    ]);
    s.label("have");
    s.emit(&[0x8e, 0xc0, 0xc3]);
    for (name, store) in [("fetch", false), ("save", true)] {
        frame(&mut s, name);
        s.emit(&[0x8b, 0x46, if store { 10 } else { 8 }]);
        s.relative(0xe8, "bank");
        s.emit(&[0x8b, 0x5e, if store { 8 } else { 6 }, 0x39, 0xd3]);
        s.branch(0x77, "bad_bank");
        if store {
            s.emit(&[0x8b, 0x46, 6, 0x26, 0x88, 7]);
        } else {
            s.emit(&[0x31, 0xc0, 0x26, 0x8a, 7]);
        }
        s.emit(&[0x5d, 0xcb]);
    }
    // Return the requested argument pointer in SI, or zero when absent.
    s.label("argv");
    s.emit(&[0x31, 0xf6, 0x83, 0xf8, 63]);
    s.branch(0x77, "argv_end");
    s.emit(&[0x89, 0xc3, 0xd1, 0xe3, 0x8b, 0xb7, 0x80, 0]);
    s.label("argv_end");
    s.emit(&[0xc3]);
    frame(&mut s, "arg");
    s.emit(&[0x8b, 0x46, 8]);
    s.relative(0xe8, "argv");
    s.emit(&[0x31, 0xc0, 0x09, 0xf6]);
    s.branch(0x74, "arg_end");
    s.emit(&[0x8b, 0x4e, 6]);
    s.label("arg_char");
    s.emit(&[0xac, 0x08, 0xc0]);
    s.branch(0x74, "arg_end");
    s.emit(&[0x09, 0xc9]);
    s.branch(0x74, "arg_end");
    s.emit(&[0x49]);
    s.relative(0xe9, "arg_char");
    s.label("arg_end");
    s.emit(&[0x5d, 0xcb]);
    for (name, write) in [("input", false), ("output", true), ("append", true)] {
        frame(&mut s, name);
        s.emit(&[0x8b, 0x46, if write { 8 } else { 6 }]);
        s.relative(0xe8, "bank");
        // Keep the bank limit in DI across filename lookup and DOS open.
        s.emit(&[0x89, 0xd7]);
        if write {
            s.emit(&[0x8b, 0x4e, 6, 0x49, 0x83, 0x7e, 6, 0]);
            s.branch(0x74, &format!("{name}/size_ok"));
            s.emit(&[0x39, 0xf9]);
            s.branch(0x77, "bad_bank");
            s.label(&format!("{name}/size_ok"));
        }
        s.emit(&[0x8b, 0x46, if write { 10 } else { 8 }]);
        s.relative(0xe8, "argv");
        s.emit(&[0x89, 0xf2, 0x09, 0xd2]);
        s.branch(0x74, "bad_arg");
        if name == "append" {
            s.emit(&[0xb8, 2, 0x3d]);
        } else if write {
            s.emit(&[0x31, 0xc9, 0xb4, 0x3c]);
        } else {
            s.emit(&[0xb8, 0, 0x3d]);
        }
        s.emit(&[0xcd, 0x21]);
        s.branch(0x72, "bad_io");
        s.emit(&[0x89, 0xc3]);
        if name == "append" {
            s.emit(&[0xb8, 2, 0x42, 0x31, 0xc9, 0x31, 0xd2, 0xcd, 0x21]);
            s.branch(0x72, "bad_io");
        }
        s.emit(&[0x1e, 0x06, 0x1f, 0x31, 0xd2]);
        if write {
            s.emit(&[0x8b, 0x4e, 6, 0xb4, 0x40]);
        } else {
            s.emit(&[0x89, 0xf9, 0xb4, 0x3f]);
        }
        s.emit(&[0xcd, 0x21, 0x1f]);
        s.branch(0x72, "bad_io");
        if write {
            s.emit(&[0x39, 0xc8]);
            s.branch(0x75, "bad_io");
        } else {
            s.emit(&[0x39, 0xc8]);
            s.branch(0x74, "bad_io");
            s.emit(&[0x89, 0xc7, 0x26, 0xc6, 5, 0]);
        }
        s.emit(&[0x50, 0xb4, 0x3e, 0xcd, 0x21]);
        s.branch(0x72, "bad_io");
        s.emit(&[0x58]);
        if write {
            s.emit(&[0x31, 0xc0]);
        }
        s.emit(&[0x5d, 0xcb]);
    }
    frame(&mut s, "putch");
    s.emit(&[
        0x8b, 0x56, 6, 0xb4, 2, 0xcd, 0x21, 0x8b, 0x46, 6, 0x5d, 0xcb,
    ]);
    frame(&mut s, "fail");
    s.emit(&[0x8b, 0x46, 6, 0xb4, 0x4c, 0xcd, 0x21]);
    s.public("rust16_abort");
    s.emit(&[0xb8, 1, 0x4c, 0xcd, 0x21]);
    crate::literals::emit(&mut s);
    s.finish()
}
fn frame(s: &mut Segment, name: &str) {
    s.public(name);
    s.emit(&[0x55, 0x89, 0xe5]);
}
