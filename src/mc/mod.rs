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

// Forge (FML) clients and BungeeCord-style forwarding append NUL-separated
// data to the server address field of the handshake packet (e.g.
// "host\0FML3\0"). Route on the part before the first NUL, like BungeeCord
// and Velocity do. The raw handshake bytes are forwarded to the backend
// unchanged, so Forge negotiation still works.
fn strip_address_suffix(address: Bytes) -> Bytes {
    match address.iter().position(|&b| b == 0) {
        Some(nul_pos) => address.slice(..nul_pos),
        None => address,
    }
}

// SRV resolution can leave a trailing root dot on the hostname.
fn strip_trailing_dot(address: Bytes) -> Bytes {
    match address.last() {
        Some(b'.') => address.slice(..address.len() - 1),
        _ => address,
    }
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

    let address = read_string_bytes(&mut buf).ok()?;
    Some(strip_trailing_dot(strip_address_suffix(address)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio_util::bytes::{BufMut, BytesMut};

    fn write_varint(buf: &mut BytesMut, mut value: u32) {
        loop {
            let byte = (value & 0x7F) as u8;
            value >>= 7;
            if value == 0 {
                buf.put_u8(byte);
                break;
            }
            buf.put_u8(byte | 0x80);
        }
    }

    fn handshake(address: &[u8]) -> Bytes {
        let mut body = BytesMut::new();
        write_varint(&mut body, 0x00); // packet id
        write_varint(&mut body, 763); // protocol version
        write_varint(&mut body, address.len() as u32);
        body.put_slice(address);
        body.put_u16(25565); // port
        write_varint(&mut body, 2); // next state: login

        let mut packet = BytesMut::new();
        write_varint(&mut packet, body.len() as u32);
        packet.extend_from_slice(&body);
        packet.freeze()
    }

    #[test]
    fn vanilla_address() {
        let name = parse_server_name(handshake(b"mc.example.com")).unwrap();
        assert_eq!(&name[..], b"mc.example.com");
    }

    #[test]
    fn forge_markers_are_stripped() {
        for suffix in [
            b"\0FML\0".as_slice(),
            b"\0FML2\0",
            b"\0FML3\0",
            b"\0FORGE",
            b"\0FORGE1",
        ] {
            let address = [b"mc.example.com".as_slice(), suffix].concat();
            let name = parse_server_name(handshake(&address)).unwrap();
            assert_eq!(&name[..], b"mc.example.com");
        }
    }

    #[test]
    fn trailing_dot_is_stripped() {
        let name = parse_server_name(handshake(b"mc.example.com.\0FML3\0")).unwrap();
        assert_eq!(&name[..], b"mc.example.com");
    }

    #[test]
    fn forwarding_payload_is_stripped() {
        let address = b"mc.example.com\0127.0.0.1\0069a79f44-4531-4d1b-9d3c-1046a94c1c5c";
        let name = parse_server_name(handshake(address)).unwrap();
        assert_eq!(&name[..], b"mc.example.com");
    }

    #[test]
    fn non_handshake_packet_is_rejected() {
        let mut packet = BytesMut::new();
        write_varint(&mut packet, 2);
        write_varint(&mut packet, 0x01); // not a handshake
        write_varint(&mut packet, 0);
        assert!(parse_server_name(packet.freeze()).is_none());
    }
}
