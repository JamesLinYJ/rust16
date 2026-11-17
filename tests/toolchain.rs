use std::{fs, path::Path, process::Command};

#[test]
fn mir_jump_relaxation_preserves_8086_ranges_and_omf_positions() {
    use rust16::machine::Segment;
    for gap in [127, 128] {
        let mut code = Segment::default();
        code.branch(0x74, "dest");
        code.emit(&vec![0x90; gap]);
        code.label("dest");
        code.public("callee");
        code.far_call("external");
        code.relax_branches().unwrap();
        let len = if gap == 127 { 2 } else { 5 };
        let expected: &[u8] = if gap == 127 {
            &[0x74, 127]
        } else {
            &[0x75, 3, 0xe9, 128, 0]
        };
        assert_eq!(&code.bytes[..len], expected);
        assert_eq!(code.publics[0].1, gap + len);
        assert_eq!(code.calls[0].0, gap + len + 1);
    }
    for gap in [126, 127] {
        let mut code = Segment::default();
        code.label("start");
        code.emit(&vec![0x90; gap]);
        code.relative(0xe9, "start");
        code.relax_branches().unwrap();
        assert_eq!(
            &code.bytes[gap..],
            if gap == 126 {
                &[0xeb, 0x80][..]
            } else {
                &[0xe9, 0x7e, 0xff][..]
            }
        );
    }
    let mut cascade = Segment::default();
    cascade.branch(0x75, "end");
    cascade.emit(&[0x90; 124]);
    cascade.branch(0x74, "end");
    cascade.label("end");
    cascade.relax_branches().unwrap();
    assert_eq!(&cascade.bytes[..2], &[0x75, 124]);
    assert_eq!(cascade.bytes.len(), 126);
}

#[test]
fn bp_displacements_and_word_immediates_use_signed_byte_boundaries() {
    let mut code = rust16::machine::Segment::default();
    for offset in [-129i16, -128, 0, 127, 128] {
        code.bp_operand(0x8d, 3, offset);
        code.arithmetic_word(0xc4, offset as u16);
    }
    assert_eq!(
        code.bytes,
        [
            0x8d, 0x9e, 0x7f, 0xff, 0x81, 0xc4, 0x7f, 0xff, 0x8d, 0x5e, 0x80, 0x83, 0xc4, 0x80,
            0x8d, 0x5e, 0x00, 0x83, 0xc4, 0x00, 0x8d, 0x5e, 0x7f, 0x83, 0xc4, 0x7f, 0x8d, 0x9e,
            0x80, 0x00, 0x81, 0xc4, 0x80, 0x00,
        ]
    );
}

fn compile(source: &Path, object: &Path, cpu: &str) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_rust16"))
        .arg(cpu)
        .arg(source)
        .arg("-o")
        .arg(object)
        .output()
        .unwrap()
}

#[test]
fn cpu_encodings_and_linker_diagnostics() {
    let dir = std::env::temp_dir().join(format!("rust16-test-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let src = dir.join("program.rs");
    let obj = dir.join("program.obj");
    let rt = dir.join("rt.obj");
    let exe = dir.join("program.exe");
    fs::write(&rt, rust16::runtime::object().unwrap()).unwrap();
    fs::write(&src,"fn compare(a:u16,b:u16)->bool{return a<b;} pub fn main16()->u16{if compare(2,5){return 7;}else{return 4;}}").unwrap();
    let mut previous = Vec::new();
    for cpu in ["-m8086", "-m8088", "-m286", "-m386"] {
        assert!(compile(&src, &obj, cpu).status.success());
        let image = fs::read(&obj).unwrap();
        let mut p = 0;
        let mut code = Vec::new();
        while p < image.len() {
            let n = u16::from_le_bytes([image[p + 1], image[p + 2]]) as usize;
            let record = &image[p..p + 3 + n];
            assert_eq!(record.iter().fold(0u8, |sum, x| sum.wrapping_add(*x)), 0);
            if record[0] == 0xa0 {
                code.extend_from_slice(&record[6..record.len() - 1]);
            }
            p += 3 + n;
        }
        assert_eq!(
            code[0],
            if cpu == "-m286" || cpu == "-m386" {
                0xc8
            } else {
                0x55
            }
        );
        assert_eq!(
            code.windows(3).any(|v| v == [0x0f, 0x92, 0xc0]),
            cpu == "-m386"
        );
        if cpu == "-m8088" {
            assert_eq!(previous, image);
        }
        previous = image;
    }
    let link = || {
        Command::new(env!("CARGO_BIN_EXE_link16"))
            .arg(&obj)
            .arg(&rt)
            .arg("-o")
            .arg(&exe)
            .output()
            .unwrap()
    };
    assert!(link().status.success());
    let mz = fs::read(&exe).unwrap();
    assert_eq!(&mz[..2], b"MZ");
    assert!(u16::from_le_bytes([mz[6], mz[7]]) >= 2);
    previous[3] ^= 1;
    fs::write(&obj, &previous).unwrap();
    assert_eq!(link().status.code(), Some(51));
    assert_eq!(mz, fs::read(&exe).unwrap());
    for (text, code) in [
        (
            "fn f(x:bool)->u16{return 1;} fn main16()->u16{return f(3);}",
            38,
        ),
        ("fn f(x:u16)->u16{if x==1{return 2;}}", 44),
        ("fn f()->bool{return true+false;}", 38),
        ("fn f()->u16{let n:u16=1;n=2;return n;}", 39),
    ] {
        fs::write(&src, text).unwrap();
        assert_eq!(compile(&src, &obj, "-m286").status.code(), Some(code));
    }
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn linker_preserves_far_relocation_across_third_image_bank() {
    use rust16::machine::Segment;
    let dir = std::env::temp_dir().join(format!("rust16-bank-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let full = dir.join("full.obj");
    let short = dir.join("short.obj");
    for (path, count) in [(&full, 40960), (&short, 40944)] {
        let mut pad = Segment::default();
        pad.emit(&vec![0xa5; count]);
        fs::write(path, pad.finish().unwrap()).unwrap();
    }
    let tail = dir.join("tail.obj");
    let mut code = Segment::default();
    code.public("_start");
    code.emit(&[0x90; 12]);
    code.far_call("destination");
    code.public("destination");
    code.emit(&[0xcb]);
    fs::write(&tail, code.finish().unwrap()).unwrap();
    let exe = dir.join("cross.exe");
    let link = |pads: &[&std::path::PathBuf]| {
        Command::new(env!("CARGO_BIN_EXE_link16"))
            .args(pads)
            .arg(&tail)
            .arg("-o")
            .arg(&exe)
            .status()
            .unwrap()
    };
    assert!(link(&[&full, &full, &short]).success());
    let mz = fs::read(&exe).unwrap();
    let w = |i: usize| u16::from_le_bytes([mz[i], mz[i + 1]]);
    let header = usize::from(w(8)) * 16;
    assert_eq!((w(6), w(28), w(30)), (1, 15, 7679));
    assert_eq!(mz.len(), header + 122896);
    assert_eq!(
        &mz[header + 122876..header + 122882],
        &[0x9a, 17, 0, 0xff, 0x1d, 0xcb]
    );
    assert!(mz[header..header + 122864].iter().all(|&x| x == 0xa5));
    assert_eq!(link(&[&full, &full, &full, &full, &short]).code(), Some(53));
    assert_eq!(fs::read(&exe).unwrap(), mz);
    fs::remove_dir_all(dir).unwrap();
}
