use std::{fs, path::Path, process::Command};

#[test]
fn bp_displacements_and_word_immediates_use_signed_byte_boundaries() {
    let mut code = rust16::machine::Segment::default();
    for offset in [-129i16, -128, 0, 127, 128] {
        code.bp_operand(0x8d, 3, offset);
        code.arithmetic_word(0xc4, offset as u16);
    }
    assert_eq!(code.bytes, [
        0x8d, 0x9e, 0x7f, 0xff, 0x81, 0xc4, 0x7f, 0xff,
        0x8d, 0x5e, 0x80, 0x83, 0xc4, 0x80,
        0x8d, 0x5e, 0x00, 0x83, 0xc4, 0x00,
        0x8d, 0x5e, 0x7f, 0x83, 0xc4, 0x7f,
        0x8d, 0x9e, 0x80, 0x00, 0x81, 0xc4, 0x80, 0x00,
    ]);
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
