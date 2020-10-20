use std::io::{self, Read};

use crate::{
    data::{Address, GameId, GameListing, GameOptions, Languages, ServerInfo},
    reader::{Data, Deserialize, PacketRead, PacketReader, PacketWriter, Serialize},
};

use log::warn;
use num_traits::FromPrimitive;

use super::{GameData, Lobby, PlayerControl, PlayerPhysics, PlayerTransform, VoteBanSystem, World};

#[derive(Debug)]
pub enum Packet {
    HostingGame {
        game_id: GameId,
    },
    Disconnected(DisconnectReason),
    PlayerJoined {
        game_id: GameId,
        player_id: i32,
        host_id: i32,
    },
    PlayerLeft {
        game_id: GameId,
        player_id: i32,
        host_id: i32,
        reason: Option<u8>,
    },
    ClientJoinedGame(JoinedGamePacket),
    GameList(GameListPacket),
    ServerList(ServerListPacket),
    GameAltered {
        game_id: GameId,
        is_public: bool,
    },
    GameStarted,
    ChangeServer {
        address: Address,
    },
    GameInfo {
        game_id: GameId,
        data: Vec<GameInfo>,
    },
    GameInfoTo {
        game_id: GameId,
        client_id: i32,
        data: Vec<GameInfo>,
    },
    KickPlayer {
        game_id: GameId,
        player_id: i32,
        ban: bool,
    },
    NotImplemented(PacketType),
    UnknownTag(u8),
}

#[derive(Debug, Copy, Clone, FromPrimitive)]
pub enum PacketType {
    HostingGame = 0x00,
    GameJoinDisconnect = 0x01,
    GameStarted = 0x02,
    PlayerLeft = 0x04,
    GameInfo = 0x05,
    GameInfoTo = 0x06,
    JoinedGame = 0x07,
    AlterGameInfo = 0x0a,
    KickPlayer = 0x0b,
    ChangeServer = 0x0d,
    ServerList = 0x0e,
    GameList = 0x10,
}

impl Serialize for Packet {
    fn serialize(&self, w: &mut PacketWriter) {
        match self {
            Packet::GameInfoTo {
                game_id,
                client_id,
                data,
            } => {
                w.write(game_id);
                w.write_i32_encoded(*client_id);
                for info in data {
                    w.write(info);
                }
            }
            Packet::GameInfo { game_id, data } => {
                w.write(game_id);
                for info in data {
                    w.write(info);
                }
            }
            Packet::KickPlayer {
                game_id,
                player_id,
                ban,
            } => {
                w.write(game_id);
                w.write_i32_encoded(*player_id);
                w.write_bool(*ban);
            }
            _ => todo!(),
        }
    }
}

impl Deserialize for Packet {
    fn deserialize<T: PacketRead + Read>(r: &mut PacketReader<T>) -> io::Result<Self> {
        let (tag, mut r) = r.read_message()?;
        Ok(match PacketType::from_u8(tag) {
            Some(PacketType::HostingGame) => Packet::HostingGame { game_id: r.read()? },
            Some(PacketType::GameStarted) => Packet::GameStarted,
            Some(PacketType::GameJoinDisconnect) => {
                // Packet type depends on how large the first int is
                // They could have just used a different packet but this is more fun
                let value = r.read_i32()?;
                if value < 0xff && value >= 0 {
                    Packet::Disconnected(DisconnectReason::from_value_and_reader(value, &mut r)?)
                } else {
                    let game_id = GameId { id: value };
                    let player_id = r.read_i32()?;
                    let host_id = r.read_i32()?;
                    Packet::PlayerJoined {
                        game_id,
                        player_id,
                        host_id,
                    }
                }
            }
            Some(PacketType::PlayerLeft) => Packet::PlayerLeft {
                game_id: r.read::<GameId>()?,
                player_id: r.read_i32()?,
                host_id: r.read_i32()?,
                reason: {
                    if r.remaining() > 0 {
                        Some(r.read_u8()?)
                    } else {
                        None
                    }
                },
            },
            Some(PacketType::JoinedGame) => Packet::ClientJoinedGame(r.read::<JoinedGamePacket>()?),
            Some(PacketType::AlterGameInfo) => {
                let game_id = r.read::<GameId>()?;
                let to_alter = r.read_u8()?;
                assert_eq!(to_alter, 1);
                let is_public = r.read_bool()?;
                Packet::GameAltered { game_id, is_public }
            }
            Some(PacketType::ChangeServer) => Packet::ChangeServer {
                address: r.read::<Address>()?,
            },
            Some(PacketType::ServerList) => Packet::ServerList(r.read::<ServerListPacket>()?),
            Some(PacketType::GameList) => Packet::GameList(r.read::<GameListPacket>()?),
            Some(PacketType::GameInfoTo) => Packet::GameInfoTo {
                game_id: r.read::<GameId>()?,
                client_id: r.read_i32_encoded()?,
                data: r.read_all::<GameInfo>()?,
            },
            Some(PacketType::GameInfo) => Packet::GameInfo {
                game_id: r.read::<GameId>()?,
                data: r.read_all::<GameInfo>()?,
            },
            Some(packet_type) => {
                warn!("Unread packet type {:?}", packet_type);
                Packet::NotImplemented(packet_type)
            }
            None => {
                warn!("Unknown packet type: {:x?}", tag);
                Packet::UnknownTag(tag)
            }
        })
    }
}

#[derive(Debug)]
pub struct GenericMessage {
    pub tag: u8,
    pub data: Box<dyn Serialize>,
}

impl Serialize for GenericMessage {
    fn serialize(&self, w: &mut PacketWriter) {
        w.start_message(self.tag);
        self.data.serialize(w);
        w.end_message();
    }
}

#[derive(Debug)]
pub enum GameInfo {
    UpdateData {
        net_id: u32,
        data: Data,
    },
    RPC {
        net_id: u32,
        call_id: u8,
        data: Data,
    },
    Destroy {
        net_id: u32,
    },
    ChangeScene {
        client_id: i32,
        scene: String,
    },
    ClientReady {
        client_id: i32,
    },
    CreateFromPrefab {
        spawn_flags: u8,
        prefab: Prefab,
    },
    Unknown,
}

impl Serialize for GameInfo {
    fn serialize(&self, w: &mut PacketWriter) {
        match self {
            GameInfo::ChangeScene { client_id, scene } => {
                w.start_message(GameInfoType::ChangeScene as u8);
                w.write_i32_encoded(*client_id);
                w.write_string(scene);
                w.end_message();
            }
            GameInfo::ClientReady { client_id } => {
                w.start_message(GameInfoType::ClientReady as u8);
                w.write_i32_encoded(*client_id);
                w.end_message();
            }
            GameInfo::RPC {
                net_id,
                call_id,
                data,
            } => {
                w.start_message(GameInfoType::RPC as u8);
                w.write_u32_encoded(*net_id);
                w.write_u8(*call_id);
                w.write(data);
                w.end_message();
            }
            GameInfo::Destroy { net_id } => {
                w.start_message(GameInfoType::Destroy as u8);
                w.write_u32_encoded(*net_id);
                w.end_message();
            }
            _ => todo!(),
        }
    }
}

impl Deserialize for GameInfo {
    fn deserialize<T: PacketRead>(r: &mut PacketReader<T>) -> io::Result<Self> {
        let (tag, mut r) = r.read_message()?;
        Ok(match GameInfoType::from_u8(tag) {
            Some(GameInfoType::UpdateData) => GameInfo::UpdateData {
                net_id: r.read_u32_encoded()?,
                data: Data::Bytes(r.remaining_bytes()?),
            },
            Some(GameInfoType::RPC) => GameInfo::RPC {
                net_id: r.read_u32_encoded()?,
                call_id: r.read_u8()?,
                data: Data::Bytes(r.remaining_bytes()?),
            },
            Some(GameInfoType::Destroy) => GameInfo::Destroy {
                net_id: r.read_u32_encoded()?,
            },
            Some(GameInfoType::ChangeScene) => GameInfo::ChangeScene {
                client_id: r.read_i32_encoded()?,
                scene: r.read_string()?,
            },
            Some(GameInfoType::ClientReady) => GameInfo::ClientReady {
                client_id: r.read_i32_encoded()?,
            },
            Some(GameInfoType::CreateFromPrefab) => {
                let prefab_id = r.read_u32_encoded()?;
                let owner_id = r.read_i32_encoded()?;
                let spawn_flags = r.read_u8()?;
                let num_children = r.read_u32_encoded()?;
                let prefab = match PrefabType::from_u32(prefab_id) {
                    Some(PrefabType::World) => {
                        assert_eq!(num_children, 1);
                        let net_id = r.read_u32_encoded()?;
                        let (tag, mut data) = r.read_message()?;
                        assert_eq!(tag, 1);
                        let map = World::initialize(net_id, owner_id, &mut data)?;
                        Prefab::World(map)
                    }
                    Some(PrefabType::Player) => {
                        assert_eq!(num_children, 3);
                        let net_id = r.read_u32_encoded()?;
                        let (tag, mut data) = r.read_message()?;
                        assert_eq!(tag, 1);
                        let player_control =
                            PlayerControl::initialize(net_id, owner_id, &mut data)?;

                        let net_id = r.read_u32_encoded()?;
                        let (tag, mut data) = r.read_message()?;
                        assert_eq!(tag, 1);
                        let player_physics =
                            PlayerPhysics::initialize(net_id, owner_id, &mut data)?;

                        let net_id = r.read_u32_encoded()?;
                        let (tag, mut data) = r.read_message()?;
                        assert_eq!(tag, 1);
                        let player_transform =
                            PlayerTransform::initialize(net_id, owner_id, &mut data)?;

                        Prefab::Player(player_control, player_physics, player_transform)
                    }
                    Some(PrefabType::Lobby) => {
                        assert_eq!(num_children, 1);
                        let net_id = r.read_u32_encoded()?;
                        let (tag, mut data) = r.read_message()?;
                        assert_eq!(tag, 1);
                        Prefab::Lobby(Lobby::initialize(net_id, owner_id, &mut data))
                    }
                    Some(PrefabType::GameData) => {
                        assert_eq!(num_children, 2);
                        let net_id = r.read_u32_encoded()?;
                        let (tag, mut data) = r.read_message()?;
                        assert_eq!(tag, 1);
                        let game_data = GameData::initialize(net_id, owner_id, &mut data)?;
                        let net_id = r.read_u32_encoded()?;
                        let (tag, mut data) = r.read_message()?;
                        assert_eq!(tag, 1);
                        let vote_ban = VoteBanSystem::initialize(net_id, owner_id, &mut data)?;
                        Prefab::GameData(game_data, vote_ban)
                    }
                    None => {
                        warn!("Unkown prefab id {}", prefab_id);
                        Prefab::Unknown
                    }
                    Some(prefab_type) => {
                        warn!("Unread prefab type {:?}", prefab_type);
                        Prefab::Unknown
                    }
                };
                GameInfo::CreateFromPrefab {
                    spawn_flags,
                    prefab,
                }
            }
            None => {
                warn!("Unknown game info type {}", tag);
                GameInfo::Unknown
            }
        })
    }
}

#[derive(Debug, Copy, Clone, FromPrimitive)]
pub enum GameInfoType {
    UpdateData = 1,
    RPC = 2,
    CreateFromPrefab = 4,
    Destroy = 5,
    ChangeScene = 6,
    ClientReady = 7,
}

#[derive(Debug, Copy, Clone, FromPrimitive)]
pub enum PrefabType {
    World = 0x00,
    MeetingHub = 0x01,
    Lobby = 0x02,
    GameData = 0x03,
    Player = 0x04,
    HeadQuarters = 0x05,
}

// TODO: Improve name. If it's been initialized it's not really a prefab
#[derive(Debug)]
pub enum Prefab {
    World(World),
    Player(PlayerControl, PlayerPhysics, PlayerTransform),
    Lobby(Lobby),
    GameData(GameData, VoteBanSystem),
    Unknown,
}

#[derive(Debug)]
pub struct JoinGamePacket {
    pub game_id: GameId,
    pub maps_owned: u8,
}

impl Serialize for JoinGamePacket {
    fn serialize(&self, w: &mut PacketWriter) {
        w.write(self.game_id);
        w.write_u8(self.maps_owned);
    }
}

#[derive(Debug)]
pub struct JoinedGamePacket {
    pub game_id: GameId,
    pub client_id: i32,
    pub host_id: i32,
    pub player_ids: Vec<i32>,
}

impl Deserialize for JoinedGamePacket {
    fn deserialize<T: PacketRead + Read>(r: &mut PacketReader<T>) -> io::Result<Self> {
        Ok(Self {
            game_id: r.read()?,
            client_id: r.read_i32()?,
            host_id: r.read_i32()?,
            player_ids: (0..r.read_u32_encoded()?)
                .map(|_| r.read_i32_encoded())
                .collect::<io::Result<_>>()?,
        })
    }
}

#[derive(Debug)]
pub struct ServerListPacket {
    pub servers: Vec<ServerInfo>,
}

impl Deserialize for ServerListPacket {
    fn deserialize<T: PacketRead + Read>(r: &mut PacketReader<T>) -> io::Result<Self> {
        assert_eq!(r.read_u8()?, 1);
        let count = r.read_u32_encoded()?;
        let servers = (0..count)
            .map(|_| {
                let (tag, mut inner_data) = r.read_message()?;
                assert_eq!(tag, 0);
                inner_data.read::<ServerInfo>()
            })
            .collect::<io::Result<_>>()?;
        Ok(Self { servers })
    }
}

#[derive(Debug)]
pub struct GameListPacket {
    pub games: Vec<GameListing>,
}

impl Deserialize for GameListPacket {
    fn deserialize<T: PacketRead + Read>(r: &mut PacketReader<T>) -> io::Result<Self> {
        let mut games = Vec::new();
        let (tag, mut inner_data) = r.read_message()?;
        assert_eq!(tag, 0);
        while inner_data.remaining() != 0 {
            let (list_tag, mut list_data) = inner_data.read_message()?;
            assert_eq!(list_tag, 0);
            games.push(list_data.read::<GameListing>()?);
        }

        Ok(Self { games })
    }
}

#[derive(Debug)]
pub enum DisconnectReason {
    ExitGame,
    GameFull,
    GameStarted,
    GameNotFound,
    IncorrectVersion,
    Banned,
    Kicked,
    Custom { message: String },
    Destroy,
    Error,
    IncorrectGame,
    ServerRequest,
    ServerFull,
    IntentionalLeaving,
    FocusLostBackground,
    FocusLost,
    NewConnection,
}

impl DisconnectReason {
    fn from_value_and_reader<T: PacketRead>(
        value: i32,
        r: &mut PacketReader<T>,
    ) -> io::Result<Self> {
        Ok(match value {
            0 => DisconnectReason::ExitGame,
            1 => DisconnectReason::GameFull,
            2 => DisconnectReason::GameStarted,
            3 => DisconnectReason::GameNotFound,
            5 => DisconnectReason::IncorrectVersion,
            6 => DisconnectReason::Banned,
            7 => DisconnectReason::Kicked,
            8 => DisconnectReason::Custom {
                message: r.read_string()?,
            },
            16 => DisconnectReason::Destroy,
            17 => DisconnectReason::Error,
            18 => DisconnectReason::IncorrectGame,
            19 => DisconnectReason::ServerRequest,
            20 => DisconnectReason::ServerFull,
            207 => DisconnectReason::FocusLostBackground,
            208 => DisconnectReason::IntentionalLeaving,
            209 => DisconnectReason::FocusLost,
            210 => DisconnectReason::NewConnection,
            _ => unreachable!(),
        })
    }
}

#[derive(Debug)]
pub struct RequestGameListPacket {
    game_options: GameOptions,
}

impl RequestGameListPacket {
    pub fn new(language: Languages, maps: u8, num_imposters: i8) -> Self {
        Self {
            game_options: GameOptions {
                language,
                map_id: maps,
                num_imposters,
                ..GameOptions::default()
            },
        }
    }
}

impl Serialize for RequestGameListPacket {
    fn serialize(&self, w: &mut PacketWriter) {
        w.write_u8(0);
        let mut inner_w = PacketWriter::new();
        inner_w.write(&self.game_options);
        w.write_u32_encoded(inner_w.len() as u32);
        w.write_bytes_raw(&inner_w.finish());
    }
}
