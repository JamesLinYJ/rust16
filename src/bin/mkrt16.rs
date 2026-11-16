fn main() {
    let path = std::env::args_os().nth(1).expect("usage: mkrt16 RT.OBJ");
    std::fs::write(path, rust16::runtime::object().expect("encode DOS runtime"))
        .expect("write RT.OBJ");
}
