//! Kitty graphics protocol encoding. Only what ttyp needs: upload an RGBA
//! image once, place it (many times) at a cell plus pixel offset, and
//! delete placements or images by id.
//! Spec: https://sw.kovidgoyal.net/kitty/graphics-protocol/

use std::io::Write;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use flate2::Compression;
use flate2::write::ZlibEncoder;

/// Max base64 payload per escape sequence, per the spec.
const CHUNK: usize = 4096;

/// Upload image `id` (`w`×`h` RGBA) without displaying it.
pub fn transmit(out: &mut Vec<u8>, id: u32, w: u32, h: u32, rgba: &[u8]) {
    debug_assert_eq!(rgba.len(), (w * h * 4) as usize);
    let mut z = ZlibEncoder::new(Vec::with_capacity(rgba.len() / 8), Compression::fast());
    z.write_all(rgba).expect("writing to a Vec cannot fail");
    let packed = z.finish().expect("writing to a Vec cannot fail");
    let data = STANDARD.encode(packed);

    let chunks: Vec<&[u8]> = data.as_bytes().chunks(CHUNK).collect();
    let last = chunks.len().saturating_sub(1);
    for (i, chunk) in chunks.iter().enumerate() {
        let more = u8::from(i < last);
        if i == 0 {
            let _ = write!(
                out,
                "\x1b_Ga=t,f=32,o=z,t=d,s={w},v={h},i={id},q=2,m={more};"
            );
        } else {
            let _ = write!(out, "\x1b_Gm={more};");
        }
        out.extend_from_slice(chunk);
        out.extend_from_slice(b"\x1b\\");
    }
}

/// Show image `id` as placement `pid`, its top-left `x_off`/`y_off` pixels
/// into cell (`col`, `row`). Moves the cursor: wrap a batch in
/// `SAVE_CURSOR` / `RESTORE_CURSOR`.
pub fn place(out: &mut Vec<u8>, id: u32, pid: u32, col: u32, row: u32, x_off: u32, y_off: u32) {
    let _ = write!(
        out,
        "\x1b[{};{}H\x1b_Ga=p,i={id},p={pid},X={x_off},Y={y_off},C=1,q=2\x1b\\",
        row + 1,
        col + 1
    );
}

/// Remove one placement, keeping the image for reuse.
pub fn delete_placement(out: &mut Vec<u8>, id: u32, pid: u32) {
    let _ = write!(out, "\x1b_Ga=d,d=i,i={id},p={pid},q=2\x1b\\");
}

/// Delete image `id` and all its placements, freeing its data.
pub fn delete(out: &mut Vec<u8>, id: u32) {
    let _ = write!(out, "\x1b_Ga=d,d=I,i={id},q=2\x1b\\");
}

pub const SAVE_CURSOR: &[u8] = b"\x1b7";
pub const RESTORE_CURSOR: &[u8] = b"\x1b8";

/// Begin/end a synchronized update so the terminal paints text and images
/// in one go.
pub const SYNC_BEGIN: &[u8] = b"\x1b[?2026h";
pub const SYNC_END: &[u8] = b"\x1b[?2026l";

#[cfg(test)]
mod tests {
    use super::*;

    fn text(out: &[u8]) -> String {
        String::from_utf8(out.to_vec()).unwrap()
    }

    #[test]
    fn place_and_delete_encode_ids_and_offsets() {
        let mut out = Vec::new();
        place(&mut out, 7, 3, 4, 2, 5, 0);
        assert_eq!(
            text(&out),
            "\x1b[3;5H\x1b_Ga=p,i=7,p=3,X=5,Y=0,C=1,q=2\x1b\\"
        );
        out.clear();
        delete_placement(&mut out, 7, 3);
        assert_eq!(text(&out), "\x1b_Ga=d,d=i,i=7,p=3,q=2\x1b\\");
    }

    #[test]
    fn small_transmit_is_one_chunk() {
        let mut out = Vec::new();
        transmit(&mut out, 9, 1, 1, &[1, 2, 3, 4]);
        let s = text(&out);
        assert!(s.starts_with("\x1b_Ga=t,f=32,o=z,t=d,s=1,v=1,i=9,q=2,m=0;"));
        assert_eq!(s.matches("\x1b_G").count(), 1);
    }

    #[test]
    fn large_payload_is_chunked() {
        // Noise defeats compression, forcing several chunks.
        let mut x: u32 = 1;
        let rgba: Vec<u8> = (0..40_000)
            .map(|_| {
                x ^= x << 13;
                x ^= x >> 17;
                x ^= x << 5;
                x as u8
            })
            .collect();
        let mut out = Vec::new();
        transmit(&mut out, 1, 100, 100, &rgba);
        let s = text(&out);
        let parts = s.matches("\x1b_G").count();
        assert!(parts > 1);
        assert_eq!(
            s.matches(",m=1;").count() + s.matches("\x1b_Gm=1;").count(),
            parts - 1
        );
        assert_eq!(s.matches("\x1b_Gm=0;").count(), 1);
        for seg in s.split("\x1b_G").skip(1) {
            let payload = seg.split(';').nth(1).unwrap().split('\x1b').next().unwrap();
            assert!(payload.len() <= CHUNK);
        }
    }
}
