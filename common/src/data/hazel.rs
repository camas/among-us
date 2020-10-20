use std::io;

use crate::reader::{Deserialize, PacketRead, PacketReader, PacketWriter, Serialize};
use num_traits::FromPrimitive;

#[derive(Debug)]
pub enum HazelPacket {
    Unreliable { data: Vec<u8> },
    Reliable { ack_id: u16, data: Vec<u8> },
    Disconnect,
    Hello { ack_id: u16, data: Vec<u8> },
    Acknowledge { ack_id: u16 },
    KeepAlive { ack_id: u16 },
}

#[derive(Debug)]
pub enum HazelPacketOut {
    Unreliable {
        data: Box<dyn Serialize>,
    },
    Reliable {
        ack_id: u16,
        data: Box<dyn Serialize>,
    },
    Disconnect,
    Hello {
        ack_id: u16,
        data: Box<dyn Serialize>,
    },
    Acknowledge {
        ack_id: u16,
    },
    KeepAlive {
        ack_id: u16,
    },
}

impl Serialize for HazelPacketOut {
    fn serialize(&self, w: &mut PacketWriter) {
        match self {
            HazelPacketOut::Unreliable { data } => {
                w.write_u8(HazelType::Unreliable as u8);
                data.serialize(w);
            }
            HazelPacketOut::Reliable { ack_id, data } => {
                w.write_u8(HazelType::Reliable as u8);
                w.write_u16_be(*ack_id);
                data.serialize(w);
            }
            HazelPacketOut::Disconnect => {
                w.write_u8(HazelType::Disconnect as u8);
            }
            HazelPacketOut::Hello { ack_id, data } => {
                w.write_u8(HazelType::Hello as u8);
                w.write_u16_be(*ack_id);
                data.serialize(w);
            }
            HazelPacketOut::Acknowledge { ack_id } => {
                w.write_u8(HazelType::Acknowledge as u8);
                w.write_u16_be(*ack_id);
                // TODO: Check this byte out properly
                w.write_u8(0x00);
            }
            HazelPacketOut::KeepAlive { ack_id } => {
                w.write_u8(HazelType::KeepAlive as u8);
                w.write_u16_be(*ack_id);
            }
        }
    }
}

impl Deserialize for HazelPacket {
    fn deserialize<T: PacketRead>(r: &mut PacketReader<T>) -> io::Result<Self> {
        let packet_type = r.read_u8()?;
        Ok(match HazelType::from_u8(packet_type) {
            Some(HazelType::Unreliable) => HazelPacket::Unreliable {
                data: r.remaining_bytes()?,
            },
            Some(HazelType::Reliable) => HazelPacket::Reliable {
                ack_id: r.read_u16_be()?,
                data: r.remaining_bytes()?,
            },
            Some(HazelType::Hello) => HazelPacket::Hello {
                ack_id: r.read_u16_be()?,
                data: r.remaining_bytes()?,
            },
            Some(HazelType::Disconnect) => HazelPacket::Disconnect,
            Some(HazelType::Acknowledge) => HazelPacket::Acknowledge {
                ack_id: r.read_u16_be()?,
            },
            Some(HazelType::KeepAlive) => HazelPacket::KeepAlive {
                ack_id: r.read_u16_be()?,
            },
            None => panic!("Unknown packet type"),
        })
    }
}

#[derive(Debug, Copy, Clone, FromPrimitive)]
pub enum HazelType {
    Unreliable = 0,
    Reliable = 1,
    Hello = 8,
    Disconnect = 9,
    Acknowledge = 10,
    KeepAlive = 12,
}
