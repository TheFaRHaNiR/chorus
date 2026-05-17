use crate::protocol::codec::RakCodec;
use crate::util::constants::MAGIC;
use crate::util::packet_id::UNCONNECTED_PONG;
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use std::io::{Error, ErrorKind, Read, Write};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnconnectedPong {
    pub timestamp: u64,
    pub guid: u64,
    pub message: Vec<u8>,
}

impl UnconnectedPong {
    pub fn new(timestamp: u64, guid: u64, message: Vec<u8>) -> Self {
        Self { timestamp, guid, message }
    }
}

impl RakCodec<UnconnectedPong> for UnconnectedPong {
    fn serialize<W: Write>(value: &Self, writer: &mut W) -> Result<(), Error> {
        writer.write_u8(UNCONNECTED_PONG)?;
        writer.write_u64::<BigEndian>(value.timestamp)?;
        writer.write_u64::<BigEndian>(value.guid)?;
        writer.write_all(&MAGIC)?;
        writer.write_u16::<BigEndian>(value.message.len() as u16)?;
        writer.write_all(&value.message)?;

        Ok(())
    }

    fn deserialize<R: Read>(reader: &mut R) -> Result<Self, Error> {
        let id = reader.read_u8()?;
        if id != UNCONNECTED_PONG {
            return Err(Error::new(ErrorKind::InvalidData, "not an UnconnectedPong"));
        }

        let timestamp = reader.read_u64::<BigEndian>()?;
        let guid = reader.read_u64::<BigEndian>()?;

        let mut magic = [0u8; MAGIC.len()];
        reader.read_exact(&mut magic)?;
        if magic != MAGIC {
            return Err(Error::new(ErrorKind::InvalidData, "invalid magic"));
        }

        let message_len = reader.read_u16::<BigEndian>()?;
        let mut message = vec![0u8; message_len as usize];
        reader.read_exact(&mut message)?;

        Ok(Self { timestamp, guid, message })
    }

    fn size_hint(value: &Self) -> usize {
        size_of::<u8>() + size_of::<u64>() + size_of::<u64>() + MAGIC.len() + size_of::<u16>() + value.message.len()
    }
}
