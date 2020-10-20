use std::{
    io::{self, Read},
    net::SocketAddr,
};

pub use hazel::*;
pub use netobjects::*;
pub use objects::*;
pub use packets::*;

use crate::reader::{Deserialize, PacketRead, PacketReader};

mod hazel;
mod netobjects;
mod objects;
mod packets;

impl Deserialize for SocketAddr {
    fn deserialize<T: PacketRead + Read>(r: &mut PacketReader<T>) -> io::Result<Self> {
        Ok(SocketAddr::from((
            [r.read_u8()?, r.read_u8()?, r.read_u8()?, r.read_u8()?],
            r.read_u16()?,
        )))
    }
}
