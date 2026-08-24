use std::fs;

fn fail(code: i32) -> ! {
    std::process::exit(code)
}

fn main() {
    let mode = fs::read_to_string("/workspace/input.txt").unwrap_or_else(|_| fail(10));

    match mode.trim() {
        "ro" => {}
        "rw" => {
            fs::write("/workspace/output.txt", b"blob-write-ok\n").unwrap_or_else(|_| fail(20));
        }
        "escape" => {
            if fs::read_to_string("/workspace/../outside.txt").is_ok() {
                fail(30);
            }
        }
        _ => fail(40),
    }
}
