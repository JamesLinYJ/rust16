use std::{env, fs, process::ExitCode};
fn main() -> ExitCode {
    let args: Vec<_> = env::args().skip(1).collect();
    let output = match args.as_slice() {
        [flag, out] if flag == "--mbr" || flag == "--vbr" => {
            rust16::boot_record::record(flag == "--mbr").map(|b| (out, b))
        }
        [flag, seg, out] if flag == "--vbr-at" => u16::from_str_radix(seg, 16)
            .map_err(|e| e.to_string())
            .and_then(|p| {
                if (0x100..=0x300).contains(&p) {
                    rust16::boot_record::record_at(false, p)
                } else {
                    Err("invalid IO.SYS load paragraph".into())
                }
            })
            .map(|b| (out, b)),
        [flag, out] if flag == "--dos-io" => rust16::firmware::utilities().map(|b| (out, b)),
        [out] => rust16::firmware::object(true).map(|b| (out, b)),
        [flag, out] if flag == "--loader" => rust16::firmware::object(false).map(|b| (out, b)),
        [out, count] => count
            .parse::<u8>()
            .map_err(|e| e.to_string())
            .and_then(rust16::firmware::sector)
            .map(|b| (out, b)),
        _ => Err("usage: bios16 OUTPUT [STAGE-SECTORS]".into()),
    };
    match output.and_then(|(out, b)| fs::write(out, b).map_err(|e| e.to_string())) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}
