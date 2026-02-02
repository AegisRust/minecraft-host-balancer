use tokio::io;
use tokio_util::bytes::{Buf, Bytes};

use crate::mc::{read_string, read_varint};

#[derive(Debug)]
pub struct Handshake {
    pub protocol_version: i32,
    pub server_address: String,
    pub server_port: u16,
    pub next_state: i32,
}

impl Handshake {
    pub fn parse_handshake(mut data: Bytes) -> io::Result<Handshake> {
        let _packet_len = read_varint(&mut data)?;

        let packet_id = read_varint(&mut data)?;
        if packet_id != 0x00 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "not a handshake packet",
            ));
        }

        let protocol_version = read_varint(&mut data)?;

        let server_address = read_string(&mut data)?;

        if data.remaining() < 2 {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "missing port"));
        }
        let server_port = data.get_u16();

        let next_state = read_varint(&mut data)?;

        Ok(Self {
            protocol_version,
            server_address,
            server_port,
            next_state,
        })
    }
}
