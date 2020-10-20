use std::{
    convert::TryInto,
    fmt::{Display, Formatter},
    io::{self, ErrorKind, Read},
    net::SocketAddr,
};

use crate::reader::{Deserialize, PacketRead, PacketReader, PacketWriter, Serialize};

bitflags! {
    pub struct Languages: u32 {
        const ALL = 0x0;
        const OTHER = 0x1;
        const SPANISH = 0x2;
        const KOREAN = 0x4;
        const RUSSIAN = 0x8;
        const PORTUGUESE = 0x10;
        const ARABIC = 0x20;
        const FILIPINO = 0x40;
        const POLISH = 0x80;
        const ENGLISH = 0x100;
    }
}

/// The 6/4 char ID used for Among Us games. Restricted to only upper-case characters (`'A'` - `'Z'`) by
/// the UI but not always by the game
///
/// V2 codes have a negative underlying value
///
/// TODO: Implement V1
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct GameId {
    pub id: i32,
}

const CHAR_LOOKUP: [char; 26] = [
    'Q', 'W', 'X', 'R', 'T', 'Y', 'L', 'P', 'E', 'S', 'D', 'F', 'G', 'H', 'U', 'J', 'K', 'Z', 'O',
    'C', 'V', 'B', 'I', 'N', 'M', 'A',
];

impl GameId {
    pub fn from_chars(chars: &str) -> Self {
        if chars.len() == 6 {
            let indexes: Vec<i32> = chars
                .chars()
                .map(|c| CHAR_LOOKUP.iter().position(|other| other == &c).unwrap() as i32)
                .collect();
            let mut lower = indexes[1];
            lower *= 26;
            lower += indexes[0];
            let mut upper = indexes[5];
            upper *= 26;
            upper += indexes[4];
            upper *= 26;
            upper += indexes[3];
            upper *= 26;
            upper += indexes[2];
            let id = lower | (upper << 10) | std::i32::MIN;
            Self { id }
        } else if chars.len() == 4 {
            let id = chars
                .chars()
                .enumerate()
                .fold(0, |acc, (index, c)| acc | (c as i32) << (8 * index));
            Self { id }
        } else {
            panic!("wrong number of chars in game id")
        }
    }
}

impl Serialize for GameId {
    fn serialize(&self, w: &mut PacketWriter) {
        w.write_i32(self.id);
    }
}

impl Deserialize for GameId {
    fn deserialize<T: PacketRead + Read>(r: &mut PacketReader<T>) -> io::Result<Self> {
        Ok(GameId { id: r.read_i32()? })
    }
}

impl Display for GameId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if self.id < -1 {
            // V2
            let upper_half = (self.id >> 10) & 0xfffff;
            let indexes = [
                (self.id & 0x3ff) % 26,
                ((self.id & 0x3ff) / 0x1a) % 26,
                upper_half % 26,
                (upper_half / 0x1a) % 26,
                (upper_half / 0x2a4) % 26,
                (upper_half / 0x44a8) % 26,
            ];
            let chars: String = indexes.iter().map(|&i| CHAR_LOOKUP[i as usize]).collect();

            write!(f, "{}", chars)
        } else {
            // V1
            let chars = [
                self.id & 255,
                (self.id >> 8) & 255,
                (self.id >> 16) & 255,
                (self.id >> 24) & 255,
            ]
            .iter()
            .map(|&index| index as u8 as char)
            .collect::<String>();
            write!(f, "{}", chars)
        }
    }
}

bitflags! {
    pub struct Maps: u8 {
        const SKELD = 0x0;
        const PORUS = 0x1;
        const MIRA_HQ = 0x2;
    }
}

#[derive(Debug, Clone)]
pub struct Address {
    pub ip: [u8; 4],
    pub port: u16,
}

impl Address {
    pub fn to_sock_add(&self) -> SocketAddr {
        SocketAddr::from((self.ip, self.port))
    }
}

impl Display for Address {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let value = format!(
            "{}.{}.{}.{}:{}",
            self.ip[0], self.ip[1], self.ip[2], self.ip[3], self.port
        );
        std::fmt::Display::fmt(&value, f)
    }
}

impl Deserialize for Address {
    fn deserialize<T: PacketRead + Read>(r: &mut PacketReader<T>) -> io::Result<Self> {
        Ok(Address {
            ip: r.read_slice(4)?.try_into().unwrap(),
            port: r.read_u16()?,
        })
    }
}

#[derive(Debug, Clone)]
pub struct GameListing {
    pub address: Address,
    pub id: GameId,
    pub host_username: String,
    pub player_count: u8,
    pub age: u32,
    pub map_id: Maps,
    pub num_imposters: u8,
    pub max_players: u8,
}

impl Deserialize for GameListing {
    fn deserialize<T: PacketRead + Read>(r: &mut PacketReader<T>) -> io::Result<Self> {
        Ok(Self {
            address: r.read::<Address>()?,
            id: r.read::<GameId>()?,
            host_username: r.read_string()?,
            player_count: r.read_u8()?,
            age: r.read_u32_encoded()?,
            map_id: Maps::from_bits(r.read_u8()?)
                .ok_or_else(|| io::Error::new(ErrorKind::InvalidData, "Invalid map bit"))?,
            num_imposters: r.read_u8()?,
            max_players: r.read_u8()?,
        })
    }
}

#[derive(Debug)]
pub struct GameOptions {
    pub game_settings_version: u8,
    pub max_players: u8,
    pub language: Languages,
    pub map_id: u8,
    pub player_speed: f32,
    pub crew_light: f32,
    pub imposter_light: f32,
    pub kill_cooldown: f32,
    pub num_common_tasks: u8,
    pub num_long_tasks: u8,
    pub num_short_tasks: u8,
    pub num_emergency_meetings: i32,
    pub num_imposters: i8,
    pub kill_distance: i8,
    pub discussion_time: i32,
    pub voting_time: i32,
    pub is_defaults: u8,
    pub emergency_cooldown: u8,
}

impl Default for GameOptions {
    fn default() -> Self {
        Self {
            game_settings_version: 2,
            max_players: 10,
            language: Languages::ENGLISH,
            map_id: 0,
            player_speed: 1.,
            crew_light: 1.,
            imposter_light: 1.5,
            kill_cooldown: 15.,
            num_common_tasks: 1,
            num_short_tasks: 1,
            num_long_tasks: 2,
            num_emergency_meetings: 1,
            num_imposters: 0, // Any
            kill_distance: 1,
            discussion_time: 15,
            voting_time: 120,
            is_defaults: 1,
            emergency_cooldown: 15,
        }
    }
}

impl Deserialize for GameOptions {
    fn deserialize<T: PacketRead + Read>(r: &mut PacketReader<T>) -> io::Result<Self> {
        Ok(Self {
            game_settings_version: r.read_u8()?,
            max_players: r.read_u8()?,
            language: Languages::from_bits(r.read_u32()?).unwrap(),
            map_id: r.read_u8()?,
            player_speed: r.read_f32()?,
            crew_light: r.read_f32()?,
            imposter_light: r.read_f32()?,
            kill_cooldown: r.read_f32()?,
            num_common_tasks: r.read_u8()?,
            num_long_tasks: r.read_u8()?,
            num_short_tasks: r.read_u8()?,
            num_emergency_meetings: r.read_i32()?,
            num_imposters: r.read_i8()?,
            kill_distance: r.read_i8()?,
            discussion_time: r.read_i32()?,
            voting_time: r.read_i32()?,
            is_defaults: r.read_u8()?,
            emergency_cooldown: r.read_u8()?,
        })
    }
}

impl Serialize for &GameOptions {
    fn serialize(&self, w: &mut PacketWriter) {
        w.write_u8(self.game_settings_version);
        w.write_u8(self.max_players);
        w.write_u32(self.language.bits());
        w.write_u8(self.map_id);
        w.write_f32(self.player_speed);
        w.write_f32(self.crew_light);
        w.write_f32(self.imposter_light);
        w.write_f32(self.kill_cooldown);
        w.write_u8(self.num_common_tasks);
        w.write_u8(self.num_long_tasks);
        w.write_u8(self.num_short_tasks);
        w.write_i32(self.num_emergency_meetings);
        w.write_i8(self.num_imposters);
        w.write_i8(self.kill_distance);
        w.write_i32(self.discussion_time);
        w.write_i32(self.voting_time);
        w.write_u8(self.is_defaults);
        w.write_u8(self.emergency_cooldown);
    }
}

#[derive(Debug)]
pub struct ServerInfo {
    pub name: String,
    pub ip: [u8; 4],
    pub port: u16,
    pub connection_failures: u32,
}

impl Deserialize for ServerInfo {
    fn deserialize<T: PacketRead + Read>(r: &mut PacketReader<T>) -> io::Result<Self> {
        Ok(ServerInfo {
            name: r.read_string()?,
            ip: r
                .read_slice(4)?
                .try_into()
                .expect("Couldn't convert bytes to ip address"),
            port: r.read_u16()?,
            connection_failures: r.read_u32_encoded()?,
        })
    }
}

#[derive(Debug, Copy, Clone, Default)]
pub struct Vector2 {
    x: f32,
    y: f32,
}

impl Vector2 {
    pub const ZERO: Vector2 = Vector2 { x: 0., y: 0. };

    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

impl Deserialize for Vector2 {
    fn deserialize<T: PacketRead + Read>(r: &mut PacketReader<T>) -> io::Result<Self> {
        let v = r.read_u16()? as f32 / 65535.;
        let v2 = r.read_u16()? as f32 / 65535.;
        let x = (v.max(0.).min(1.) * 80.) - 40.;
        let y = (v2.max(0.).min(1.) * 80.) - 40.;
        Ok(Self { x, y })
    }
}

impl Serialize for Vector2 {
    fn serialize(&self, w: &mut PacketWriter) {
        let v = ((self.x / 80.) + 40.).max(0.).min(1.);
        let v2 = ((self.y / 80.) + 40.).max(0.).min(1.);
        w.write_u16((v * 65555.) as u16);
        w.write_u16((v2 * 65555.) as u16);
    }
}

#[derive(Debug)]
pub struct PlayerData {
    pub name: String,
    pub color: u8,
    pub hat_id: u32,
    pub skin_id: u32,
    pub pet_id: u32,
    pub disconnected: bool,
    pub is_imposter: bool,
    pub is_dead: bool,
    pub tasks: Vec<TaskInfo>,
    pub dirty: bool,
}

impl PlayerData {
    pub fn update_data<T: PacketRead>(&mut self, r: &mut PacketReader<T>) -> io::Result<()> {
        *self = r.read()?;
        Ok(())
    }
}

impl Serialize for PlayerData {
    fn serialize(&self, w: &mut PacketWriter) {
        w.write_string(&self.name);
        w.write_u8(self.color);
        w.write_u32_encoded(self.hat_id);
        w.write_u32_encoded(self.skin_id);
        w.write_u32_encoded(self.pet_id);
        let flags = if self.disconnected { 1 } else { 0 }
            | if self.is_imposter { 2 } else { 0 }
            | if self.is_dead { 4 } else { 0 };
        w.write_u8(flags);
        w.write_u8(self.tasks.len() as u8);
        self.tasks.iter().for_each(|task| w.write(task));
    }
}

impl Deserialize for PlayerData {
    #[allow(clippy::eval_order_dependence)] // Shh
    fn deserialize<T: PacketRead>(r: &mut PacketReader<T>) -> io::Result<Self> {
        let flags;
        Ok(Self {
            dirty: false,
            name: r.read_string()?,
            color: r.read_u8()?,
            hat_id: r.read_u32_encoded()?,
            skin_id: r.read_u32_encoded()?,
            pet_id: r.read_u32_encoded()?,
            disconnected: {
                flags = r.read_u8()?;
                flags & 1 > 0
            },
            is_imposter: flags & 2 > 0,
            is_dead: flags & 4 > 0,
            tasks: {
                let count = r.read_u8()?;
                (0..count)
                    .map(|_| r.read::<TaskInfo>())
                    .collect::<io::Result<_>>()?
            },
        })
    }
}

#[derive(Debug)]
pub struct TaskInfo {
    id: u32,
    complete: bool,
}

impl Serialize for TaskInfo {
    fn serialize(&self, w: &mut PacketWriter) {
        w.write_u32_encoded(self.id);
        w.write_bool(self.complete);
    }
}

impl Deserialize for TaskInfo {
    fn deserialize<T: PacketRead + Read>(r: &mut PacketReader<T>) -> io::Result<Self> {
        Ok(Self {
            id: r.read_u32_encoded()?,
            complete: r.read_bool()?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rayon::prelude::*;

    #[test]
    fn test_known_gameid() {
        let code = "AQNKQQ";
        let id = GameId::from_chars(code);
        assert_eq!(&id.id.to_le_bytes(), &[0x19, 0xdc, 0x06, 0x80]);
    }

    /// Will take too long if not in release as this iterates through all ~300 million (26**6) possible game codes
    #[test]
    fn test_gameid_all_nums() {
        (b'A'..=b'Z')
            .into_par_iter()
            .map(char::from)
            .for_each(|c_1| {
                for c_2 in (b'A'..=b'Z').map(char::from) {
                    for c_3 in (b'A'..=b'Z').map(char::from) {
                        for c_4 in (b'A'..=b'Z').map(char::from) {
                            for c_5 in (b'A'..=b'Z').map(char::from) {
                                for c_6 in (b'A'..=b'Z').map(char::from) {
                                    let original =
                                        format!("{}{}{}{}{}{}", c_1, c_2, c_3, c_4, c_5, c_6);
                                    let id = GameId::from_chars(&original);
                                    let back = id.to_string();
                                    assert_eq!(original, back);
                                }
                            }
                        }
                    }
                }
            });
    }
}
