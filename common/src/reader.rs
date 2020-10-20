use std::{
    collections::VecDeque,
    fmt::Debug,
    io::Cursor,
    io::{self, ErrorKind, Read, Result, Seek, SeekFrom, Write},
};

/// A binary reader that mimics the .NET `BinaryReader`
#[derive(Debug)]
pub struct PacketReader<T: PacketRead> {
    data: T,
}

impl<T: PacketRead> PacketReader<T> {
    pub fn new(data: T) -> Self {
        Self { data }
    }

    /// Reads a message, returning the tag and a reader over the message data
    ///
    /// Reads a u16 length and a u8 tag
    #[inline]
    pub fn read_message(&mut self) -> Result<(u8, PacketReader<&[u8]>)> {
        let length = self.read_u16()?;
        let tag = self.read_u8()?;
        let data = self.read_slice(length as usize)?;
        Ok((tag, PacketReader::new(data)))
    }

    /// Reads `count` number of bytes
    #[inline]
    pub fn read_bytes_raw(&mut self, count: usize) -> Result<Vec<u8>> {
        let mut vec = vec![0; count];
        self.data.read_exact(&mut vec)?;
        Ok(vec)
    }

    /// Reads a deserializeable object
    #[inline]
    pub fn read<S: Deserialize>(&mut self) -> Result<S> {
        S::deserialize(self)
    }

    /// Reads a packed u32 and then `Vec` with that length of type `S`
    #[inline]
    pub fn read_vec<S: Deserialize>(&mut self) -> Result<Vec<S>> {
        let count = self.read_u32_encoded()?;
        (0..count).map(|_| self.read::<S>()).collect()
    }

    /// Reads the specified type until no data left
    #[inline]
    pub fn read_all<S: Deserialize>(&mut self) -> Result<Vec<S>> {
        let mut result = Vec::new();
        while self.data.remaining() != 0 {
            result.push(self.read::<S>()?);
        }
        Ok(result)
    }

    /// Reads a bool encoded as a single byte
    #[inline]
    pub fn read_bool(&mut self) -> Result<bool> {
        Ok(match self.read_u8()? {
            0 => false,
            1 => true,
            value => panic!("Unexpected value for read_bool {}", value),
        })
    }

    /// Reads a u8
    #[inline]
    pub fn read_u8(&mut self) -> Result<u8> {
        let mut buf = [0; 1];
        self.data.read_exact(&mut buf)?;
        Ok(buf[0])
    }

    /// Reads a u16
    #[inline]
    pub fn read_u16(&mut self) -> Result<u16> {
        let mut buf = [0; 2];
        self.data.read_exact(&mut buf)?;
        Ok(u16::from_le_bytes(buf))
    }

    /// Reads a big endian u16
    #[inline]
    pub fn read_u16_be(&mut self) -> Result<u16> {
        let mut buf = [0; 2];
        self.data.read_exact(&mut buf)?;
        Ok(u16::from_be_bytes(buf))
    }

    /// Reads a u32
    #[inline]
    pub fn read_u32(&mut self) -> Result<u32> {
        let mut buf = [0; 4];
        self.data.read_exact(&mut buf)?;
        Ok(u32::from_le_bytes(buf))
    }

    /// Reads an i8
    #[inline]
    pub fn read_i8(&mut self) -> Result<i8> {
        let mut buf = [0; 1];
        self.data.read_exact(&mut buf)?;
        Ok(i8::from_le_bytes(buf))
    }

    /// Reads an i16
    #[inline]
    pub fn read_i16(&mut self) -> Result<i16> {
        let mut buf = [0; 2];
        self.data.read_exact(&mut buf)?;
        Ok(i16::from_le_bytes(buf))
    }

    /// Reads an i32
    #[inline]
    pub fn read_i32(&mut self) -> Result<i32> {
        let mut buf = [0; 4];
        self.data.read_exact(&mut buf)?;
        Ok(i32::from_le_bytes(buf))
    }

    /// Reads an f32
    #[inline]
    pub fn read_f32(&mut self) -> Result<f32> {
        let mut buf = [0; 4];
        self.data.read_exact(&mut buf)?;
        Ok(f32::from_le_bytes(buf))
    }

    /// Reads a packed u32
    ///
    /// See <https://docs.microsoft.com/en-us/openspecs/sharepoint_protocols/ms-spptc/1eeaf7cc-f60b-4144-aa12-4eb9f6e748d1>
    #[inline]
    pub fn read_u32_encoded(&mut self) -> Result<u32> {
        let mut value: u32 = 0;
        for offset in (0..).step_by(7) {
            let byte = self.read_u8()?;
            value |= ((byte & 127) as u32) << offset;
            // Return if "read next" bit unset or if 5 bytes read
            if (byte & 128) == 0 || offset > 28 {
                return Ok(value);
            }
        }
        unreachable!()
    }

    /// Reads a packed i32
    ///
    /// See <https://docs.microsoft.com/en-us/openspecs/sharepoint_protocols/ms-spptc/1eeaf7cc-f60b-4144-aa12-4eb9f6e748d1>
    #[inline]
    pub fn read_i32_encoded(&mut self) -> Result<i32> {
        Ok(self.read_u32_encoded()? as i32)
    }

    /// Reads a string prefixed by it's length as a packed u32
    #[inline]
    pub fn read_string(&mut self) -> Result<String> {
        let length = self.read_u32_encoded()?;
        let data = self.read_bytes_raw(length as usize)?;
        String::from_utf8(data).map_err(|str_err| io::Error::new(ErrorKind::InvalidData, str_err))
    }

    /// Returns a slice of the underlying data
    #[inline]
    pub fn read_slice(&mut self, length: usize) -> Result<&[u8]> {
        self.data.read_slice(length)
    }

    /// Returns the number of bytes unread
    #[inline]
    pub fn remaining(&mut self) -> usize {
        self.data.remaining()
    }

    /// Reads the remaining bytes
    #[inline]
    pub fn remaining_bytes(&mut self) -> Result<Vec<u8>> {
        self.data.remaining_bytes()
    }
}

impl PacketRead for Cursor<Vec<u8>> {
    #[inline]
    fn remaining(&mut self) -> usize {
        self.get_ref().len() - self.position() as usize
    }

    fn read_slice(&mut self, length: usize) -> Result<&[u8]> {
        let pos = self.position() as usize;
        self.seek(SeekFrom::Current(length as i64))?;
        Ok(&self.get_ref()[pos..(pos + length)])
    }

    fn remaining_bytes(&mut self) -> Result<Vec<u8>> {
        let mut buffer = Vec::new();
        self.read_to_end(&mut buffer)?;
        Ok(buffer)
    }
}

impl PacketRead for &[u8] {
    #[inline]
    fn remaining(&mut self) -> usize {
        self.len()
    }

    #[inline]
    fn read_slice(&mut self, length: usize) -> io::Result<&[u8]> {
        if length > self.len() {
            return Err(io::Error::new(
                ErrorKind::UnexpectedEof,
                "Tried to read out of bound slice",
            ));
        }
        let (a, b) = self.split_at(length);
        *self = b;
        Ok(a)
    }

    #[inline]
    fn remaining_bytes(&mut self) -> Result<Vec<u8>> {
        let result = self.to_vec();
        *self = &[];
        Ok(result)
    }
}

pub trait PacketRead: Read {
    fn remaining(&mut self) -> usize;

    fn read_slice(&mut self, length: usize) -> Result<&[u8]>;

    fn remaining_bytes(&mut self) -> Result<Vec<u8>>;
}

pub trait Deserialize: Sized {
    fn deserialize<T: PacketRead>(r: &mut PacketReader<T>) -> Result<Self>;
}

pub trait GetReader {
    fn get_reader(&self) -> PacketReader<&[u8]>;
}

impl GetReader for &[u8] {
    fn get_reader(&self) -> PacketReader<&[u8]> {
        PacketReader { data: self }
    }
}

pub trait IntoReader {
    fn into_reader(self) -> PacketReader<Cursor<Vec<u8>>>;
}

impl IntoReader for Vec<u8> {
    fn into_reader(self) -> PacketReader<Cursor<Vec<u8>>> {
        PacketReader {
            data: Cursor::new(self),
        }
    }
}

/// A binary writer that mimics the .NET `BinaryWriter`
#[derive(Debug)]
pub struct PacketWriter {
    data: Cursor<Vec<u8>>,
    message_starts: VecDeque<u64>,
}

impl Default for PacketWriter {
    fn default() -> Self {
        Self {
            data: Cursor::new(Vec::new()),
            message_starts: VecDeque::new(),
        }
    }
}

impl PacketWriter {
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the size of the data written so far
    #[inline]
    pub fn len(&self) -> usize {
        self.data.get_ref().len()
    }

    /// Returns true if no data written
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.data.get_ref().is_empty()
    }

    /// Writes a serializable object
    #[inline]
    pub fn write<S: Serialize>(&mut self, value: S) {
        value.serialize(self);
    }

    /// Writes a bool
    ///
    /// Encoded as a single byte where `true`: `0x01` and `false`: `0x00`
    #[inline]
    pub fn write_bool(&mut self, value: bool) {
        self.data
            .write_all(if value { &[1] } else { &[0] })
            .unwrap();
    }

    /// Writes a u8
    #[inline]
    pub fn write_u8(&mut self, value: u8) {
        self.data.write_all(&value.to_le_bytes()).unwrap();
    }

    /// Writes a u16
    #[inline]
    pub fn write_u16(&mut self, value: u16) {
        self.data.write_all(&value.to_le_bytes()).unwrap();
    }

    /// Writes a big endian u16
    #[inline]
    pub fn write_u16_be(&mut self, value: u16) {
        self.data.write_all(&value.to_be_bytes()).unwrap();
    }

    #[inline]
    pub fn write_u32(&mut self, value: u32) {
        self.data.write_all(&value.to_le_bytes()).unwrap();
    }

    /// Writes an i8
    #[inline]
    pub fn write_i8(&mut self, value: i8) {
        self.data.write_all(&value.to_le_bytes()).unwrap();
    }

    /// Writes an i16
    #[inline]
    pub fn write_i16(&mut self, value: i16) {
        self.data.write_all(&value.to_le_bytes()).unwrap();
    }

    #[inline]
    pub fn write_i32(&mut self, value: i32) {
        self.data.write_all(&value.to_le_bytes()).unwrap();
    }

    #[inline]
    pub fn write_f32(&mut self, value: f32) {
        self.data.write_all(&value.to_le_bytes()).unwrap();
    }

    #[inline]
    pub fn write_bytes_raw(&mut self, value: &[u8]) {
        self.data.write_all(value).unwrap();
    }

    /// Writes a 7 bit encoded u32
    ///
    /// See <https://docs.microsoft.com/en-us/openspecs/sharepoint_protocols/ms-spptc/1eeaf7cc-f60b-4144-aa12-4eb9f6e748d1>
    #[inline]
    pub fn write_u32_encoded(&mut self, mut value: u32) {
        while value >= 128 {
            self.write_u8(value as u8 | 128);
            value >>= 7;
        }
        self.write_u8(value as u8);
    }

    /// Writes a 7 bit encoded i32
    ///
    /// See <https://docs.microsoft.com/en-us/openspecs/sharepoint_protocols/ms-spptc/1eeaf7cc-f60b-4144-aa12-4eb9f6e748d1>
    #[inline]
    pub fn write_i32_encoded(&mut self, value: i32) {
        self.write_u32_encoded(value as u32);
    }

    /// Writes a string
    ///
    /// The length is written first as a packed u32
    #[inline]
    pub fn write_string(&mut self, value: &str) {
        self.write_u32_encoded(value.len() as u32);
        self.data.write_all(value.as_bytes()).unwrap();
    }

    /// Starts a message
    ///
    /// Make sure to call `end_message` when finished
    #[inline]
    pub fn start_message(&mut self, tag: u8) {
        self.message_starts.push_back(self.data.position());
        self.write_u16(0xFFFF); // Value is temporary
        self.write_u8(tag);
    }

    /// Ends a message, writing the length to the start of the message
    #[inline]
    pub fn end_message(&mut self) {
        let message_start = self.message_starts.pop_back().unwrap();
        let end_pos = self.data.position();
        let data_len = end_pos - message_start - 3;
        self.data.set_position(message_start);
        self.write_u16(data_len as u16);
        self.data.set_position(end_pos);
    }

    /// Returns the bytes written
    #[inline]
    pub fn finish(self) -> Vec<u8> {
        assert!(self.message_starts.is_empty());
        self.data.into_inner()
    }
}

pub trait Serialize: Debug + Send + Sync {
    fn serialize(&self, w: &mut PacketWriter);

    #[inline]
    fn serialize_bytes(&self) -> Vec<u8> {
        let mut w = PacketWriter::new();
        self.serialize(&mut w);
        w.finish()
    }
}

impl<S: Serialize> Serialize for &'_ S {
    #[inline]
    fn serialize(&self, w: &mut PacketWriter) {
        (*self).serialize(w);
    }
}

/// Container enum to make passing serializable objects or raw bytes easier
#[derive(Debug)]
pub enum Data {
    Bytes(Vec<u8>),
    Object(Box<dyn Serialize>),
}

impl Serialize for Data {
    fn serialize(&self, w: &mut PacketWriter) {
        match self {
            Data::Bytes(bytes) => w.write_bytes_raw(bytes),
            Data::Object(obj) => obj.serialize(w),
        }
    }
}

// impl IntoReader for Data {
//     fn into_reader(self) -> PacketReader<Vec<u8>> {
//         match self {
//             Data::Bytes(data) => data.into_reader(),
//             Data::Object(obj) =>
//         }
//     }
// }
