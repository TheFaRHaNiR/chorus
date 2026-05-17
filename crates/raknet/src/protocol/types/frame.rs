use crate::protocol::codec::RakCodec;
use crate::types::reliability::RakReliability;
use crate::util::flags::SPLIT;
use byteorder::{BigEndian, LittleEndian, ReadBytesExt, WriteBytesExt};
use std::io::{Error, ErrorKind, Read, Write};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Frame {
    reliability: RakReliability,
    payload: Vec<u8>,
    reliable_index: u32,
    sequence_index: u32,
    order_index: u32,
    order_channel: u8,
    split_size: u32,
    split_id: u16,
    split_index: u32,
}

impl Frame {
    pub fn new(reliability: RakReliability, payload: Vec<u8>) -> Self {
        Self {
            reliability,
            payload,
            reliable_index: 0,
            sequence_index: 0,
            order_index: 0,
            order_channel: 0,
            split_size: 0,
            split_id: 0,
            split_index: 0,
        }
    }

    pub fn is_split(&self) -> bool {
        self.split_size > 0
    }
}

impl RakCodec<Frame> for Frame {
    fn serialize<W: Write>(value: &Self, writer: &mut W) -> Result<(), Error> {
        let mut flags = (value.reliability as u8) << 5;
        if value.is_split() {
            flags |= SPLIT;
        }
        writer.write_u8(flags)?;

        writer.write_u16::<BigEndian>((value.payload.len() as u16) << 3)?;

        if value.reliability.is_reliable() {
            writer.write_u24::<LittleEndian>(value.reliable_index)?;
        }

        if value.reliability.is_sequenced() {
            writer.write_u24::<LittleEndian>(value.sequence_index)?;
        }

        if value.reliability.is_ordered() || value.reliability.is_sequenced() {
            writer.write_u24::<LittleEndian>(value.order_index)?;
            writer.write_u8(value.order_channel)?;
        }

        if value.is_split() {
            writer.write_u32::<BigEndian>(value.split_size)?;
            writer.write_u16::<BigEndian>(value.split_id)?;
            writer.write_u32::<BigEndian>(value.split_index)?;
        }

        writer.write_all(&value.payload)?;

        Ok(())
    }

    fn deserialize<R: Read>(reader: &mut R) -> Result<Self, Error> {
        let header = reader.read_u8()?;
        let reliability = RakReliability::try_from((header & 0xE0) >> 5).map_err(|_| Error::new(ErrorKind::InvalidData, "invalid reliability"))?;

        let length = (reader.read_u16::<BigEndian>()? as usize + 7) >> 3;

        let reliable_index: u32 = if reliability.is_reliable() { reader.read_u24::<LittleEndian>()? } else { 0 };

        let sequence_index: u32 = if reliability.is_sequenced() { reader.read_u24::<LittleEndian>()? } else { 0 };

        let (order_index, order_channel) = if reliability.is_ordered() || reliability.is_sequenced() {
            let order_index = reader.read_u24::<LittleEndian>()?;
            let order_channel = reader.read_u8()?;
            (order_index, order_channel)
        } else {
            (0, 0)
        };

        let (split_size, split_id, split_index) = if header & SPLIT != 0 {
            let split_size = reader.read_u32::<BigEndian>()?;
            let split_id = reader.read_u16::<BigEndian>()?;
            let split_index = reader.read_u32::<BigEndian>()?;
            (split_size, split_id, split_index)
        } else {
            (0, 0, 0)
        };

        let mut payload = Vec::with_capacity(length);
        reader.read_exact(&mut payload)?;

        Ok(Self {
            reliability,
            payload,
            reliable_index,
            sequence_index,
            order_index,
            order_channel,
            split_size,
            split_id,
            split_index,
        })
    }

    fn size_hint(value: &Self) -> usize {
        size_of::<u8>()
            + size_of::<u16>()
            + if value.reliability.is_reliable() { 3 } else { 0 }
            + if value.reliability.is_sequenced() { 3 } else { 0 }
            + if value.reliability.is_ordered() || value.reliability.is_sequenced() {
                3 + size_of::<u8>()
            } else {
                0
            }
            + if value.is_split() { size_of::<u32>() + size_of::<u16>() + size_of::<u32>() } else { 0 }
            + value.payload.len()
    }
}
