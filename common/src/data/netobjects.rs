use std::{collections::hash_map::Entry, collections::HashMap, fmt::Debug, io};

use log::{info, warn};
use num_traits::FromPrimitive;

use crate::reader::{Data, PacketRead, PacketReader, PacketWriter};

use super::{GameInfo, PlayerData, Vector2};

pub trait NetObject: Debug {
    fn update_data(&mut self, r: &mut PacketReader<&[u8]>) -> io::Result<()>;

    fn handle_rpc(&mut self, call_id: u8, r: &mut PacketReader<&[u8]>) -> io::Result<RPCCallback>;

    fn owner_id(&self) -> i32;

    fn set_owner_id(&mut self, value: i32);

    fn net_id(&self) -> u32;

    fn set_net_id(&mut self, value: u32);
}

macro_rules! net_obj_funcs {
    () => {
        fn owner_id(&self) -> i32 {
            self.owner_id
        }

        fn set_owner_id(&mut self, value: i32) {
            self.owner_id = value;
        }

        fn net_id(&self) -> u32 {
            self.net_id
        }

        fn set_net_id(&mut self, value: u32) {
            self.net_id = value;
        }
    };
}

#[derive(Debug)]
pub enum RPCCallback {
    ChatMessage { message: String },
    None,
}

#[derive(Debug)]
pub struct PlayerControl {
    net_id: u32,
    owner_id: i32,
    pub player_id: u8,
    pub name: Option<String>,
}

impl PlayerControl {
    pub fn initialize<T: PacketRead>(
        net_id: u32,
        owner_id: i32,
        r: &mut PacketReader<T>,
    ) -> io::Result<Self> {
        let _is_new = r.read_bool()?;
        Ok(Self {
            owner_id,
            net_id,
            player_id: r.read_u8()?,
            name: None,
        })
    }

    pub fn rpc_check_name(&self, name: &str) -> GameInfo {
        let mut w = PacketWriter::new();
        w.write_string(name);
        GameInfo::RPC {
            net_id: self.net_id,
            call_id: PlayerControlRPCType::CheckName as u8,
            data: Data::Bytes(w.finish()),
        }
    }

    pub fn rpc_set_name(&self, name: &str) -> GameInfo {
        let mut w = PacketWriter::new();
        w.write_string(name);
        GameInfo::RPC {
            net_id: self.net_id,
            call_id: PlayerControlRPCType::SetName as u8,
            data: Data::Bytes(w.finish()),
        }
    }

    pub fn rpc_chat_message(&self, message: &str) -> GameInfo {
        let mut w = PacketWriter::new();
        w.write_string(message);
        GameInfo::RPC {
            net_id: self.net_id,
            call_id: PlayerControlRPCType::SendChat as u8,
            data: Data::Bytes(w.finish()),
        }
    }

    pub fn rpc_check_color(&self, color_index: u8) -> GameInfo {
        let mut w = PacketWriter::new();
        w.write_u8(color_index);
        GameInfo::RPC {
            net_id: self.net_id,
            call_id: PlayerControlRPCType::CheckColor as u8,
            data: Data::Bytes(w.finish()),
        }
    }

    pub fn rpc_set_skin(&self, skin_index: u32) -> GameInfo {
        let mut w = PacketWriter::new();
        w.write_u32_encoded(skin_index);
        GameInfo::RPC {
            net_id: self.net_id,
            call_id: PlayerControlRPCType::SetSkin as u8,
            data: Data::Bytes(w.finish()),
        }
    }

    pub fn rpc_set_hat(&self, hat_index: u32) -> GameInfo {
        let mut w = PacketWriter::new();
        w.write_u32_encoded(hat_index);
        GameInfo::RPC {
            net_id: self.net_id,
            call_id: PlayerControlRPCType::SetHat as u8,
            data: Data::Bytes(w.finish()),
        }
    }

    pub fn rpc_set_pet(&self, skin_index: u32) -> GameInfo {
        let mut w = PacketWriter::new();
        w.write_u32_encoded(skin_index);
        GameInfo::RPC {
            net_id: self.net_id,
            call_id: PlayerControlRPCType::SetPet as u8,
            data: Data::Bytes(w.finish()),
        }
    }
}

#[derive(Debug, Copy, Clone, FromPrimitive)]
enum PlayerControlRPCType {
    PlayAnimation = 0,
    CompleteTask = 1,
    SetGameOptions = 2,
    SetInfected = 3,
    Exile = 4,
    CheckName = 5,
    SetName = 6,
    CheckColor = 7,
    SetColor = 8,
    SetHat = 9,
    SetSkin = 10,
    ReportBody = 11,
    MurderPlayer = 12,
    SendChat = 13,
    MeetingCalled = 14,
    SetScanner = 15,
    AddChatNote = 16,
    SetPet = 17,
    SetStartCounter = 18,
}

impl NetObject for PlayerControl {
    net_obj_funcs!();

    fn update_data(&mut self, r: &mut PacketReader<&[u8]>) -> io::Result<()> {
        self.player_id = r.read_u8()?;
        Ok(())
    }

    fn handle_rpc(&mut self, call_id: u8, r: &mut PacketReader<&[u8]>) -> io::Result<RPCCallback> {
        let call_type = match PlayerControlRPCType::from_u8(call_id) {
            Some(value) => value,
            None => {
                warn!("Unknown PlayerControl RPC call id: {}", call_id);
                return Ok(RPCCallback::None);
            }
        };
        match call_type {
            PlayerControlRPCType::PlayAnimation => {
                let animation_id = r.read_u8()?;
                info!("Playing animation {} for {}", animation_id, self.player_id);
            }
            PlayerControlRPCType::SetInfected => {
                let count = r.read_u32_encoded()?;
                for _ in 0..count {
                    let player_id = r.read_u8()?;
                    warn!("Unhandled imposter {}", player_id);
                }
            }
            PlayerControlRPCType::SendChat => {
                let message = r.read_string()?;
                return Ok(RPCCallback::ChatMessage { message });
            }
            PlayerControlRPCType::SetName => {
                let name = r.read_string()?;
                self.name = Some(name);
            }
            _ => warn!("Unread PlayerControl RPC call type: {:?}", call_type),
        }
        Ok(RPCCallback::None)
    }
}

#[derive(Debug)]
pub struct PlayerPhysics {
    net_id: u32,
    owner_id: i32,
}

impl PlayerPhysics {
    pub fn initialize<T: PacketRead>(
        net_id: u32,
        owner_id: i32,
        _r: &mut PacketReader<T>,
    ) -> io::Result<Self> {
        Ok(Self { owner_id, net_id })
    }

    pub fn rpc_enter_vent(&self, vent_id: u32) -> GameInfo {
        let mut w = PacketWriter::new();
        w.write_u32_encoded(vent_id);
        GameInfo::RPC {
            net_id: self.net_id,
            call_id: PlayerPhysicsRPCType::EnterVent as u8,
            data: Data::Bytes(w.finish()),
        }
    }

    pub fn rpc_exit_vent(&self, vent_id: u32) -> GameInfo {
        let mut w = PacketWriter::new();
        w.write_u32_encoded(vent_id);
        GameInfo::RPC {
            net_id: self.net_id,
            call_id: PlayerPhysicsRPCType::ExitVent as u8,
            data: Data::Bytes(w.finish()),
        }
    }
}

impl NetObject for PlayerPhysics {
    net_obj_funcs!();

    fn update_data(&mut self, _r: &mut PacketReader<&[u8]>) -> io::Result<()> {
        Ok(())
    }

    fn handle_rpc(&mut self, call_id: u8, r: &mut PacketReader<&[u8]>) -> io::Result<RPCCallback> {
        let call_type = PlayerPhysicsRPCType::from_u8(call_id);
        if call_type.is_none() {
            warn!("Unknown PlayerPhysics rpc type {}", call_id);
            return Ok(RPCCallback::None);
        }
        match call_type.unwrap() {
            PlayerPhysicsRPCType::EnterVent => {
                let vent_id = r.read_u32_encoded()?;
                info!(
                    "Player with owner id {} entered vent {}",
                    self.owner_id, vent_id
                );
            }
            PlayerPhysicsRPCType::ExitVent => {
                let vent_id = r.read_u32_encoded()?;
                info!(
                    "Player with owner id {} exited vent {}",
                    self.owner_id, vent_id
                );
            }
        }
        Ok(RPCCallback::None)
    }
}

#[derive(Debug, Copy, Clone, FromPrimitive)]
enum PlayerPhysicsRPCType {
    EnterVent = 0x13,
    ExitVent = 0x14,
}

#[derive(Debug)]
pub struct PlayerTransform {
    net_id: u32,
    owner_id: i32,
    pub last_seq_id: u16,
    pub target_position: Vector2,
    pub velocity: Vector2,
}

impl PlayerTransform {
    pub fn initialize<T: PacketRead>(
        net_id: u32,
        owner_id: i32,
        r: &mut PacketReader<T>,
    ) -> io::Result<Self> {
        Ok(Self {
            owner_id,
            net_id,
            last_seq_id: r.read_u16()?,
            target_position: r.read::<Vector2>()?,
            velocity: r.read::<Vector2>()?,
        })
    }

    pub fn rpc_snap_to(&mut self, new_pos: Vector2) -> GameInfo {
        let mut w = PacketWriter::new();
        w.write(new_pos);
        w.write_u16(self.last_seq_id + 5);
        GameInfo::RPC {
            net_id: self.net_id,
            call_id: PlayerTransformRPCType::SnapTo as u8,
            data: Data::Bytes(w.finish()),
        }
    }
}

impl NetObject for PlayerTransform {
    net_obj_funcs!();

    fn update_data(&mut self, r: &mut PacketReader<&[u8]>) -> io::Result<()> {
        self.last_seq_id = r.read_u16()?;
        self.target_position = r.read::<Vector2>()?;
        self.velocity = r.read::<Vector2>()?;
        Ok(())
    }

    fn handle_rpc(&mut self, call_id: u8, r: &mut PacketReader<&[u8]>) -> io::Result<RPCCallback> {
        match call_id {
            0x15 => {
                self.target_position = r.read()?;
                self.last_seq_id = r.read_u16()?;
                self.velocity = Vector2::ZERO;
            }
            _ => warn!("Unknown PlayerTransform call id: {}", call_id),
        }
        Ok(RPCCallback::None)
    }
}

#[derive(Debug, Copy, Clone, FromPrimitive)]
enum PlayerTransformRPCType {
    SnapTo = 0x15,
}

/// The game world
///
/// Also known as ShipStatus, Ship and would probably just be considered a scene in Unity
#[derive(Debug)]
pub struct World {
    net_id: u32,
    owner_id: i32,

    // Reactor
    pub reactor_countdown: f32,
    pub user_console_pairs: Vec<(u8, u8)>,

    // Switch
    pub expected_switches: u8,
    pub actual_switches: u8,
    pub elec_value: u8,

    // Life Support
    pub life_supp_countdown: f32,
    pub completed_consoles: Vec<u32>,

    // Med Scan
    pub med_user_list: Vec<i8>,

    // Security Camera
    pub camera_in_use: bool,

    // Comms
    pub comms_active: bool,

    // Doors
    pub door_open: Vec<bool>,

    // Sabotage
    pub sabotage_timer: f32,
}

impl World {
    pub fn initialize<T: PacketRead>(
        net_id: u32,
        owner_id: i32,
        r: &mut PacketReader<T>,
    ) -> io::Result<Self> {
        Ok(Self {
            net_id,
            owner_id,
            // Reactor
            reactor_countdown: r.read_f32()?,
            user_console_pairs: (0..r.read_u32_encoded()?)
                .map(|_| match (r.read_u8(), r.read_u8()) {
                    (Ok(a), Ok(b)) => Ok((a, b)),
                    (Err(a), _) => Err(a),
                    (_, Err(b)) => Err(b),
                })
                .collect::<io::Result<_>>()?,

            // Switch
            expected_switches: r.read_u8()?,
            actual_switches: r.read_u8()?,
            elec_value: r.read_u8()?,

            // Life Support
            life_supp_countdown: r.read_f32()?,
            completed_consoles: (0..r.read_u32_encoded()?)
                .map(|_| r.read_u32_encoded())
                .collect::<io::Result<_>>()?,
            // Med Scan
            med_user_list: (0..r.read_u32_encoded()?)
                .map(|_| r.read_i8())
                .collect::<io::Result<_>>()?,

            // Security Camera
            camera_in_use: r.read_bool()?,

            // Comms
            comms_active: r.read_bool()?,

            // Doors
            door_open: { (0..13).map(|_| r.read_bool()).collect::<io::Result<_>>()? },

            // Sabotage
            sabotage_timer: r.read_f32()?,
        })
    }
}

impl NetObject for World {
    net_obj_funcs!();

    fn update_data(&mut self, r: &mut PacketReader<&[u8]>) -> io::Result<()> {
        let to_update = r.read_u32_encoded()?;

        if to_update & (1 << 3) > 0 {
            self.reactor_countdown = r.read_f32()?;
            self.user_console_pairs = (0..r.read_u32_encoded()?)
                .map(|_| match (r.read_u8(), r.read_u8()) {
                    (Ok(a), Ok(b)) => Ok((a, b)),
                    (Err(a), _) => Err(a),
                    (_, Err(b)) => Err(b),
                })
                .collect::<io::Result<_>>()?;
        }

        if to_update & (1 << 7) > 0 {
            self.expected_switches = r.read_u8()?;
            self.actual_switches = r.read_u8()?;
            self.elec_value = r.read_u8()?;
        }

        if to_update & (1 << 8) > 0 {
            self.life_supp_countdown = r.read_f32()?;
            self.completed_consoles = (0..r.read_u32_encoded()?)
                .map(|_| r.read_u32_encoded())
                .collect::<io::Result<_>>()?;
        }

        if to_update & (1 << 0xa) > 0 {
            self.med_user_list = (0..r.read_u32_encoded()?)
                .map(|_| r.read_i8())
                .collect::<io::Result<_>>()?;
        }

        if to_update & (1 << 0xb) > 0 {
            self.camera_in_use = r.read_bool()?;
        }

        if to_update & (1 << 0xe) > 0 {
            self.comms_active = r.read_bool()?;
        }

        if to_update & (1 << 0x10) > 0 {
            let doors_flags = r.read_u32_encoded()?;
            for i in 0..self.door_open.len() {
                if doors_flags & (1 << i) > 0 {
                    self.door_open[i] = r.read_bool()?;
                }
            }
        }

        if to_update & (1 << 0x11) > 0 {
            self.sabotage_timer = r.read_f32()?;
        }

        Ok(())
    }

    fn handle_rpc(&mut self, call_id: u8, r: &mut PacketReader<&[u8]>) -> io::Result<RPCCallback> {
        match call_id {
            0 => {
                let room_type = r.read_u8()?;
                warn!("Unhandled door close Room Type: {}", room_type);
            }
            1 => {
                let system_type = r.read_u8()?;
                let player_net_id = r.read_u32_encoded()?;
                let amount = r.read_u8()?;
                warn!(
                    "Unhandled system repair System: {} Player NID: {} Amount: {}",
                    system_type, player_net_id, amount
                );
            }
            _ => warn!("Unknown World rpc call {}", call_id),
        }
        Ok(RPCCallback::None)
    }
}

#[derive(Debug)]
pub struct Lobby {
    net_id: u32,
    owner_id: i32,
}

impl Lobby {
    pub fn initialize<T: PacketRead>(net_id: u32, owner_id: i32, _: &mut PacketReader<T>) -> Self {
        Self { net_id, owner_id }
    }
}

impl NetObject for Lobby {
    net_obj_funcs!();

    fn update_data(&mut self, _: &mut PacketReader<&[u8]>) -> io::Result<()> {
        warn!("Unhandled Lobby data update");
        Ok(())
    }

    fn handle_rpc(&mut self, call_id: u8, _r: &mut PacketReader<&[u8]>) -> io::Result<RPCCallback> {
        warn!("Unknown Lobby RPC call {}", call_id);
        Ok(RPCCallback::None)
    }
}

#[derive(Debug)]
pub struct GameData {
    net_id: u32,
    owner_id: i32,
    pub players: HashMap<u8, PlayerData>,
}

impl GameData {
    pub fn initialize<T: PacketRead>(
        net_id: u32,
        owner_id: i32,
        data: &mut PacketReader<T>,
    ) -> io::Result<Self> {
        Ok(Self {
            net_id,
            owner_id,
            players: {
                (0..data.read_u32_encoded()?)
                    .map(|_| match (data.read_u8(), data.read()) {
                        (Ok(id), Ok(data)) => Ok((id, data)),
                        (Err(a), _) => Err(a),
                        (_, Err(b)) => Err(b),
                    })
                    .collect::<io::Result<_>>()?
            },
        })
    }

    pub fn rpc_update_player_info(&mut self) -> GameInfo {
        let mut w = PacketWriter::new();
        self.players
            .iter()
            .filter(|(_, data)| data.dirty)
            .for_each(|(id, data)| {
                w.start_message(*id);
                w.write(data);
                w.end_message();
            });
        GameInfo::RPC {
            net_id: self.net_id,
            call_id: GameDataRPCType::UpdatePlayerInfo as u8,
            data: Data::Bytes(w.finish()),
        }
    }
}

impl NetObject for GameData {
    net_obj_funcs!();

    fn update_data(&mut self, r: &mut PacketReader<&[u8]>) -> io::Result<()> {
        for _ in 0..r.read_u8()? {
            let player_id = r.read_u8()?;
            self.players.insert(player_id, r.read()?);
        }
        Ok(())
    }

    fn handle_rpc(&mut self, call_id: u8, r: &mut PacketReader<&[u8]>) -> io::Result<RPCCallback> {
        match GameDataRPCType::from_u8(call_id) {
            Some(GameDataRPCType::UpdatePlayerInfo) => {
                while r.remaining() > 0 {
                    let (tag, mut r) = r.read_message()?;
                    match self.players.entry(tag) {
                        Entry::Occupied(mut value) => {
                            value.get_mut().update_data(&mut r)?;
                        }
                        Entry::Vacant(value) => {
                            value.insert(r.read()?);
                        }
                    };
                }
            }
            Some(value) => warn!("Unhandled GameData RPC call {:?}", value),
            None => warn!("Unknown GameData RPC call {}", call_id),
        }
        Ok(RPCCallback::None)
    }
}

#[derive(Debug, Copy, Clone, FromPrimitive)]
enum GameDataRPCType {
    SetTasks = 0x1d,
    UpdatePlayerInfo = 0x1e,
}

#[derive(Debug)]
pub struct VoteBanSystem {
    net_id: u32,
    owner_id: i32,
}

impl VoteBanSystem {
    pub fn initialize(net_id: u32, owner_id: i32, r: &mut PacketReader<&[u8]>) -> io::Result<Self> {
        let mut obj = Self { net_id, owner_id };
        obj.update_data(r)?;
        Ok(obj)
    }
}

impl NetObject for VoteBanSystem {
    net_obj_funcs!();

    fn update_data(&mut self, r: &mut PacketReader<&[u8]>) -> io::Result<()> {
        let any_votes = r.read_bool()?;
        if any_votes {
            // todo!()
        }
        warn!("Unhandled VoteBanSystem data update");
        Ok(())
    }

    fn handle_rpc(&mut self, call_id: u8, _r: &mut PacketReader<&[u8]>) -> io::Result<RPCCallback> {
        warn!("Unknown VoteBanSystem RPC call {}", call_id);
        Ok(RPCCallback::None)
    }
}
