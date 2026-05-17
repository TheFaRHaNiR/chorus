use std::io::{Error, Read, Write};

pub trait RakCodec<T>: Sized {
    fn serialize<W: Write>(value: &T, writer: &mut W) -> Result<(), Error>;

    fn deserialize<R: Read>(reader: &mut R) -> Result<T, Error>;

    fn size_hint(value: &T) -> usize;
}
