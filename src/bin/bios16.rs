use std::{env, fs, process::ExitCode};
fn main() -> ExitCode {
    let args: Vec<_> = env::args().skip(1).collect();
    let output = match args.as_slice() {
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
