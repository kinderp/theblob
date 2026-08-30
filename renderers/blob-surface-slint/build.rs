use std::fs;
use std::path::Path;

fn decode_base64(input: &str) -> Result<Vec<u8>, String> {
    fn value(byte: u8) -> Option<u8> {
        match byte {
            b'A'..=b'Z' => Some(byte - b'A'),
            b'a'..=b'z' => Some(byte - b'a' + 26),
            b'0'..=b'9' => Some(byte - b'0' + 52),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    }

    let clean = input
        .bytes()
        .filter(|byte| !byte.is_ascii_whitespace())
        .collect::<Vec<_>>();

    if clean.len() % 4 != 0 {
        return Err("base64 input length must be divisible by four".into());
    }

    let mut output = Vec::with_capacity(clean.len() / 4 * 3);
    for chunk in clean.chunks_exact(4) {
        let pad2 = chunk[2] == b'=';
        let pad3 = chunk[3] == b'=';
        let a = value(chunk[0]).ok_or_else(|| "invalid base64 character".to_string())? as u32;
        let b = value(chunk[1]).ok_or_else(|| "invalid base64 character".to_string())? as u32;
        let c = if pad2 { 0 } else { value(chunk[2]).ok_or_else(|| "invalid base64 character".to_string())? as u32 };
        let d = if pad3 { 0 } else { value(chunk[3]).ok_or_else(|| "invalid base64 character".to_string())? as u32 };
        let packed = (a << 18) | (b << 12) | (c << 6) | d;
        output.push(((packed >> 16) & 0xff) as u8);
        if !pad2 {
            output.push(((packed >> 8) & 0xff) as u8);
        }
        if !pad3 {
            output.push((packed & 0xff) as u8);
        }
    }
    Ok(output)
}

fn materialize_png(name: &str) {
    let directory = Path::new("ui/assets/generated");
    let encoded_path = directory.join(format!("{name}.png.b64"));
    let png_path = directory.join(format!("{name}.png"));

    println!("cargo:rerun-if-changed={}", encoded_path.display());
    let encoded = fs::read_to_string(&encoded_path)
        .unwrap_or_else(|error| panic!("read {}: {error}", encoded_path.display()));
    let decoded = decode_base64(&encoded)
        .unwrap_or_else(|error| panic!("decode {}: {error}", encoded_path.display()));
    fs::write(&png_path, decoded)
        .unwrap_or_else(|error| panic!("write {}: {error}", png_path.display()));
}

fn main() {
    for asset in [
        "blob-dev-idle",
        "blob-docs-idle",
        "blob-system-idle",
        "blob-notes-idle",
    ] {
        materialize_png(asset);
    }

    slint_build::compile("ui/app.slint").expect("compile Blob Native Slint UI");
}
