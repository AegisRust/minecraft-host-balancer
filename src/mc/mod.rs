use tokio::io;
use tokio_util::bytes::{Buf, Bytes};

pub mod handshake;

fn read_varint(src: &mut Bytes) -> io::Result<i32> {
    let mut value = 0;
    let mut position = 0;

    loop {
        if !src.has_remaining() {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "VarInt incomplete",
            ));
        }

        let byte = src.get_u8();
        value |= ((byte & 0x7F) as i32) << position;

        if (byte & 0x80) == 0 {
            break;
        }

        position += 7;
        if position >= 32 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "VarInt is too big",
            ));
        }
    }
    Ok(value)
}

fn read_string(src: &mut Bytes) -> io::Result<String> {
    let len = read_varint(src)? as usize;

    if src.remaining() < len {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "String length mismatch",
        ));
    }

    let string_bytes = src.copy_to_bytes(len);

    String::from_utf8(string_bytes.to_vec())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Invalid UTF-8"))
}
