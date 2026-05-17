use crate::protocol::codec::RakCodec;
use crate::util::constants::MAGIC;
use crate::util::packet_id::UNCONNECTED_PING;
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use std::io::{Error, ErrorKind, Read, Write};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnconnectedPing {
    pub timestamp: u64,
    pub client: u64,
}

impl RakCodec<UnconnectedPing> for UnconnectedPing {
    fn serialize<W: Write>(value: &Self, writer: &mut W) -> Result<(), Error> {
        writer.write_u8(UNCONNECTED_PING)?;
        writer.write_u64::<BigEndian>(value.timestamp)?;
        writer.write_all(&MAGIC)?;
        writer.write_u64::<BigEndian>(value.client)?;

        Ok(())
    }

    fn deserialize<R: Read>(reader: &mut R) -> Result<Self, Error> {
        let id = reader.read_u8()?;
        if id != UNCONNECTED_PING {
            return Err(Error::new(ErrorKind::InvalidData, "not an UnconnectedPing"));
        }

        let timestamp = reader.read_u64::<BigEndian>()?;
        let mut magic = [0u8; MAGIC.len()];
        reader.read_exact(&mut magic)?;

        if magic != MAGIC {
            return Err(Error::new(ErrorKind::InvalidData, "invalid magic"));
        }

        let client = reader.read_u64::<BigEndian>()?;

        Ok(Self { timestamp, client })
    }

    fn size_hint(_: &Self) -> usize {
        size_of::<u8>() + size_of::<u64>() + MAGIC.len() + size_of::<u64>()
    }
}
