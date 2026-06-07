use tokio::io;
use tokio_util::bytes::{Buf, Bytes};

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

fn read_string_bytes(src: &mut Bytes) -> io::Result<Bytes> {
    let len = read_varint(src)? as usize;

    if src.remaining() < len {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "String length mismatch",
        ));
    }

    let string_bytes = src.copy_to_bytes(len);
    Ok(string_bytes)
}

pub fn parse_server_name(mut buf: Bytes) -> Option<Bytes> {
    let Ok(packet_len) = read_varint(&mut buf) else {
        return None;
    };

    if packet_len < 0 {
        return None;
    }

    let Ok(packet_id) = read_varint(&mut buf) else {
        return None;
    };

    if packet_id != 0x00 {
        return None;
    }

    read_varint(&mut buf).ok()?; //skip protocol version

    read_string_bytes(&mut buf).ok()
}
