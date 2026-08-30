use std::fs;
use std::path::Path;

fn decode_base64(input: &str) -> Result<Vec<u8>, String> {
    fn value(byte: u8) -> Option<u8> {
        match byte {
            b'A'..=b'Z' => Some(byte - b'A'),
            b'a'..=b'z' => Some(byte - b'a' + 26),
            b'0'..=b'9' => Some(byte - b'0' + 52),
            b'+' | b'-' => Some(62),
            b'/' | b'_' => Some(63),
            _ => None,
        }
    }

    // Normalize the transport representation first. Some connector paths may
    // preserve optional RFC 4648 padding differently; padding is therefore
    // removed here and reapplied exactly once below. Whitespace is ignored.
    let mut payload = Vec::with_capacity(input.len());
    let mut ignored = 0usize;
    for byte in input.bytes() {
        if byte.is_ascii_whitespace() || byte == b'=' {
            continue;
        }
        if value(byte).is_some() {
            payload.push(byte);
        } else {
            ignored += 1;
        }
    }

    if ignored != 0 {
        println!("cargo:warning=ignored {ignored} non-base64 transport byte(s) while materializing Blob asset");
    }

    match payload.len() % 4 {
        0 => {}
        2 => payload.extend_from_slice(b"=="),
        3 => payload.push(b'='),
        _ => return Err("invalid base64 payload length (remainder 1)".into()),
    }

    let mut output = Vec::with_capacity(payload.len() / 4 * 3);
    for chunk in payload.chunks_exact(4) {
        let pad2 = chunk[2] == b'=';
        let pad3 = chunk[3] == b'=';
        if pad2 && !pad3 {
            return Err("invalid base64 padding".into());
        }

        let a = value(chunk[0]).ok_or_else(|| "invalid base64 character in position 0".to_string())? as u32;
        let b = value(chunk[1]).ok_or_else(|| "invalid base64 character in position 1".to_string())? as u32;
        let c = if pad2 {
            0
        } else {
            value(chunk[2]).ok_or_else(|| "invalid base64 character in position 2".to_string())? as u32
        };
        let d = if pad3 {
            0
        } else {
            value(chunk[3]).ok_or_else(|| "invalid base64 character in position 3".to_string())? as u32
        };

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

    const PNG_SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";
    const PNG_IEND: &[u8; 12] = b"\x00\x00\x00\x00IEND\xaeB`\x82";
    if decoded.get(..8) != Some(PNG_SIGNATURE.as_slice()) {
        panic!("decoded {} is not a PNG", encoded_path.display());
    }
    if !decoded.ends_with(PNG_IEND) {
        panic!("decoded {} has no valid PNG IEND trailer", encoded_path.display());
    }

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
