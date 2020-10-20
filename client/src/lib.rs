use std::{
    collections::HashSet, sync::mpsc::channel, sync::mpsc::RecvTimeoutError, sync::Arc,
    sync::RwLock, time::Duration,
};

use common::{
    data::GenericMessage,
    data::RPCCallback,
    data::Vector2,
    reader::{Data, IntoReader, PacketWriter, Serialize},
};
use common::{
    data::{
        DisconnectReason, GameData, GameId, GameInfo, GameListing, HazelPacket, JoinGamePacket,
        Languages, Lobby, NetObject, Packet, PacketType, PlayerControl, PlayerPhysics,
        PlayerTransform, Prefab, RequestGameListPacket, ServerListPacket, VoteBanSystem, World,
    },
    reader::GetReader,
};
use log::{debug, error, info, warn};

pub use crate::networking::MainServer;
use crate::networking::NetClient;

mod networking;

const AMONG_US_VERSION: u32 = 50_51_65_50;

/// Misc options for the client
///
/// Sane as possible defaults
pub struct ClientSettings {
    /// The username to connect to the server with
    ///
    /// Is checked against a blacklist but isn't the same as the in game username, which
    /// isn't checked so setting to something generic works as a bypass
    pub connect_username: String,

    /// The username to use when joining the game
    ///
    /// 12 char limit
    ///
    /// Unlike `server_username`, this one isn't checked
    pub game_username: String,

    /// The color to set when joining the game
    ///
    /// The value is an offset into the following table:
    ///
    /// ```csharp
    /// public static readonly Color32[] PlayerColors = new Color32[]
    /// {
    ///     new Color32(198, 17, 17, byte.MaxValue),
    ///     new Color32(19, 46, 210, byte.MaxValue),
    ///     new Color32(17, 128, 45, byte.MaxValue),
    ///     new Color32(238, 84, 187, byte.MaxValue),
    ///     new Color32(240, 125, 13, byte.MaxValue),
    ///     new Color32(246, 246, 87, byte.MaxValue),
    ///     new Color32(63, 71, 78, byte.MaxValue),
    ///     new Color32(215, 225, 241, byte.MaxValue),
    ///     new Color32(107, 47, 188, byte.MaxValue),
    ///     new Color32(113, 73, 30, byte.MaxValue),
    ///     new Color32(56, byte.MaxValue, 221, byte.MaxValue),
    ///     new Color32(80, 240, 57, byte.MaxValue)
    /// };
    /// ```
    pub initial_color: u8,

    /// The skin to set on joining the game
    ///
    /// Value is an index
    pub initial_skin: u32,

    /// The hat to set on joining the game
    ///
    /// Value is an index
    pub initial_hat: u32,

    /// The pet to set on joining the game
    ///
    /// Value is an index
    pub initial_pet: u32,

    /// The unity scene to set the player as when joining a game
    ///
    /// Either `OnlineGame` which is the usual game or `Tutorial` which breaks online
    /// games as the host doesn't check it
    pub game_scene: String,

    /// Whether to send the scene when joining a game
    ///
    /// Required for character to appear and for the host to send initial data
    pub send_scene: bool,

    /// Whether to send username, skin, pet etc. when joining a game
    pub send_initial_info: bool,
}

impl Default for ClientSettings {
    fn default() -> Self {
        Self {
            connect_username: "client".to_string(),
            game_username: "client".to_string(),
            initial_color: 0,
            initial_hat: 0,
            initial_pet: 0,
            initial_skin: 0,
            game_scene: "OnlineGame".to_string(),
            send_scene: true,
            send_initial_info: true,
        }
    }
}

pub struct ScanSettings {
    /// The main server to query for games
    pub server: MainServer,

    /// Username to use when connecting to the server
    pub connect_username: String,

    /// The maps to include in the query
    ///
    /// Bitflags:
    /// 0x01 - Skeld
    /// 0x02 - Porus
    /// 0x04 - Mira
    pub maps: u8,

    /// The game language to query
    pub language: Languages,

    /// The number of imposters
    ///
    /// 1, 2, 3, or 0 for any
    pub num_imposters: i8,

    pub max_requests: u32,

    pub cache_size: u32,
}

impl Default for ScanSettings {
    fn default() -> Self {
        Self {
            server: MainServer::Europe,
            connect_username: "client".to_string(),
            maps: 7,
            language: Languages::ALL,
            num_imposters: 0,
            max_requests: 10,
            cache_size: 200,
        }
    }
}

pub struct Client {
    client: NetClient,
    should_disconnect: bool,
    game_id: Option<GameId>,
    pub client_id: Option<i32>,
    pub host_id: Option<i32>,
    pub player_ids: HashSet<i32>,
    pub net_objects: NetObjectHandler,
    is_public: bool,
}

impl Client {
    fn new(client: NetClient) -> Self {
        Self {
            client,
            should_disconnect: false,
            game_id: None,
            client_id: None,
            host_id: None,
            player_ids: HashSet::new(),
            net_objects: NetObjectHandler::new(),
            is_public: false,
        }
    }

    /// Returns true if in-game and host, false otherwise
    pub fn is_host(&self) -> bool {
        if self.host_id.is_none() || self.client_id.is_none() {
            return false;
        }
        self.host_id.unwrap() == self.client_id.unwrap()
    }

    /// Scan the server for game listings until the callback returns false
    pub fn server_scan<F>(settings: ScanSettings, mut callback: F)
    where
        F: FnMut(Vec<GameListing>) -> bool,
    {
        #[derive(PartialEq)]
        enum ScanState {
            Connecting,
            Sending,
        };
        let (req_send, req_recv) = channel::<bool>();

        let game_listings = Arc::new(RwLock::new(Vec::new()));

        let listings = game_listings.clone();
        // client thread so client stays connected while game listings are being parsed
        let client_thread = std::thread::spawn(move || {
            let client = NetClient::connect(settings.server).unwrap();
            let mut client = Client::new(client);

            // Hello packet
            client.send_hello(&settings.connect_username);

            let mut state = ScanState::Connecting;
            let mut reqs_sent = 0;

            // Main loop
            loop {
                // Get next packet
                let packet = client.client.read_packet();

                // Connected once any packet received
                if state == ScanState::Connecting {
                    state = ScanState::Sending;
                }

                // Send requests if needed
                if reqs_sent < settings.max_requests {
                    let num_requested = reqs_sent * 10;
                    let num_cache = listings.read().unwrap().len();
                    let num_to_req: i32 =
                        settings.cache_size as i32 - (num_requested + num_cache as u32) as i32;
                    // Divide rounding up
                    let reqs_to_make = (num_to_req + 9) as u32 / 10;
                    for _ in 0..reqs_to_make {
                        client.request_game_list(
                            settings.language,
                            settings.maps,
                            settings.num_imposters,
                        );
                    }
                    reqs_sent += reqs_to_make;
                }

                match packet {
                    // This works? Love rust
                    HazelPacket::Unreliable { data } | HazelPacket::Reliable { data, .. } => {
                        let mut r = data.into_reader();

                        // Read packets
                        let packets = r.read_all::<Packet>();
                        if let Err(packet_error) = packets {
                            error!("Error reading packets {}", packet_error);
                            continue;
                        }
                        let packets = packets.unwrap();

                        // Handle packets
                        for packet in packets {
                            match packet {
                                Packet::Disconnected(reason) => {
                                    warn!("Disconnect: {:?}", reason);
                                    return;
                                }
                                Packet::ServerList(_) => (),
                                Packet::GameList(listing_packet) => {
                                    if let Some(value) = reqs_sent.checked_sub(1) {
                                        reqs_sent = value;
                                    }
                                    listings.write().unwrap().extend(listing_packet.games);
                                }
                                _ => warn!("Unhandled packet {:?} in server_scan", packet),
                            }
                        }
                    }
                    HazelPacket::Disconnect => {
                        warn!("Hazel disconnect");
                        return;
                    }
                    HazelPacket::Acknowledge { .. }
                    | HazelPacket::KeepAlive { .. }
                    | HazelPacket::Hello { .. } => (),
                }

                // Check if should exit
                match req_recv.recv_timeout(Duration::from_millis(200)) {
                    Ok(should_continue) => {
                        if !should_continue {
                            break;
                        }
                    }
                    Err(RecvTimeoutError::Timeout) => (),
                    _ => panic!(),
                }
            }
        });

        loop {
            let values = {
                let mut listings = game_listings.write().unwrap();
                std::mem::replace(&mut *listings, Vec::new())
            };
            if values.is_empty() {
                continue;
            }
            let should_continue = callback(values);
            req_send.send(should_continue).unwrap();
            if !should_continue {
                client_thread.join().unwrap();
                return;
            }
        }
    }

    pub fn run_game_code<H: EventHandler>(
        handler: H,
        server: MainServer,
        game_code: &str,
        settings: ClientSettings,
    ) {
        let client = NetClient::connect(server).unwrap();
        Client::run_game_inner(handler, client, GameId::from_chars(game_code), settings);
    }

    pub fn run_game<H: EventHandler>(handler: H, listing: GameListing, settings: ClientSettings) {
        let client = NetClient::connect_direct(listing.address.to_sock_add()).unwrap();
        Client::run_game_inner(handler, client, listing.id, settings);
    }

    fn run_game_inner<H: EventHandler>(
        mut handler: H,
        client: NetClient,
        game_id: GameId,
        settings: ClientSettings,
    ) {
        let mut client = Client::new(client);

        // Send hello packet
        client.send_hello(&settings.connect_username);

        // Join game
        client.join_game_id(game_id);

        // Parsing loop
        loop {
            if client.should_disconnect {
                break;
            }
            let hazel_packet = client.client.read_packet();
            handler.packet_received(&mut client);

            match hazel_packet {
                HazelPacket::Unreliable { data } | HazelPacket::Reliable { data, .. } => {
                    let mut r = data.into_reader();

                    // Read packets
                    let packets = r.read_all::<Packet>();
                    if let Err(packet_error) = packets {
                        error!("Error reading packets {}", packet_error);
                        continue;
                    }
                    let packets = packets.unwrap();

                    // Handle packets
                    for packet in packets {
                        match packet {
                            Packet::Disconnected(reason) => {
                                handler.disconnect_reason(&mut client, reason)
                            }
                            Packet::ServerList(packet) => handler.server_info(&mut client, packet),
                            Packet::GameList(_listings) => warn!("Unexpected game list packet"),
                            Packet::ChangeServer { address } => {
                                client.client =
                                    NetClient::connect_direct(address.to_sock_add()).unwrap();
                                client.send_hello(&settings.connect_username);
                                client.join_game_id(game_id);
                            }
                            Packet::ClientJoinedGame(data) => {
                                client.client_id = Some(data.client_id);
                                client.host_id = Some(data.host_id);
                                client.player_ids.extend(data.player_ids.into_iter());
                                if settings.send_scene {
                                    client.change_scene(&settings.game_scene);
                                }
                            }
                            Packet::PlayerJoined {
                                game_id,
                                player_id,
                                host_id,
                            } => {
                                if game_id != client.game_id.unwrap() {
                                    continue;
                                }
                                client.player_ids.insert(player_id);
                                client.host_id = Some(host_id);
                            }
                            Packet::PlayerLeft {
                                game_id,
                                player_id,
                                host_id,
                                ..
                            } => {
                                if game_id != client.game_id.unwrap() {
                                    continue;
                                }
                                client.player_ids.remove(&player_id);
                                client.host_id = Some(host_id);
                            }
                            Packet::GameStarted => {
                                if !client.is_host() {
                                    client.send_ready();
                                }
                            }
                            Packet::GameInfo { game_id, data } => {
                                if client.game_id.is_none() || game_id != client.game_id.unwrap() {
                                    info!("Got game info for wrong game {}. Ignoring", game_id);
                                    continue;
                                }
                                Client::handle_game_info(
                                    &mut client,
                                    &mut handler,
                                    &settings,
                                    data,
                                );
                            }
                            Packet::GameInfoTo {
                                game_id,
                                client_id,
                                data,
                            } => {
                                // Check client_id is the right one
                                if let Some(my_id) = client.client_id {
                                    if my_id != client_id {
                                        info!("Got GameInfo meant for {}. Ignoring", client_id);
                                        continue;
                                    }
                                }
                                if client.game_id.is_none() || game_id != client.game_id.unwrap() {
                                    info!("Got game info for wrong game {}. Ignoring", game_id);
                                    continue;
                                }
                                Client::handle_game_info(
                                    &mut client,
                                    &mut handler,
                                    &settings,
                                    data,
                                );
                            }
                            Packet::GameAltered { game_id, is_public } => {
                                if game_id != client.game_id.unwrap() {
                                    info!(
                                        "Got game altered info for wrong game {:?}. Ignoring",
                                        packet
                                    );
                                }
                                client.is_public = is_public;
                            }
                            _ => warn!("Unhandled packet type {:?}", packet),
                        }
                    }
                }
                HazelPacket::Disconnect => {
                    if client.should_disconnect {
                        info!("Disconnected");
                        return;
                    } else {
                        info!("Disconnected. Rejoining");
                        client.send_hello(&settings.connect_username);
                        client.join_game_id(game_id);
                    }
                }
                HazelPacket::Acknowledge { .. }
                | HazelPacket::KeepAlive { .. }
                | HazelPacket::Hello { .. } => (),
            }
        }
    }

    fn handle_game_info<H: EventHandler>(
        client: &mut Client,
        handler: &mut H,
        settings: &ClientSettings,
        data: Vec<GameInfo>,
    ) {
        for info in data {
            match info {
                GameInfo::Destroy { net_id } => {
                    if !client.net_objects.remove(net_id) {
                        info!("Destroy called for unknown net object {}", net_id);
                    }
                }
                GameInfo::UpdateData { net_id, data } => {
                    if let Some(obj) = client.net_objects.get(net_id) {
                        match data {
                            Data::Bytes(data) => {
                                if let Err(read_error) =
                                    obj.update_data(&mut (&data[..]).get_reader())
                                {
                                    error!("Error updating net object data {}", read_error);
                                }
                            }
                            Data::Object(_) => todo!(),
                        }
                    } else {
                        info!("Update Data called for unknown net object {}", net_id);
                    }
                }
                GameInfo::RPC {
                    net_id,
                    call_id,
                    data,
                } => {
                    // Check net object exists
                    if let Some(obj) = client.net_objects.get(net_id) {
                        // Let net object parse/handle data
                        let data = match data {
                            Data::Bytes(data) => data,
                            Data::Object(_) => panic!("Can't unpack a serialized object"),
                        };
                        let rpc_data = obj.handle_rpc(call_id, &mut (&data[..]).get_reader());

                        // React to any callback
                        match rpc_data {
                            Ok(rpc_data) => match rpc_data {
                                RPCCallback::ChatMessage { message } => {
                                    let owner_id = obj.owner_id();
                                    handler.chat_message(client, owner_id, message);
                                }
                                RPCCallback::None => (),
                                // callback => warn!("Unhandled RPC callback {:?}", callback),
                            },
                            Err(error) => error!("Error handling net object rpc {}", error),
                        }
                    } else {
                        info!("Handle RPC called for unknown net object {}", net_id);
                    }
                }
                GameInfo::CreateFromPrefab { prefab, .. } => {
                    let is_self = if let Prefab::Player(control, _, _) = &prefab {
                        control.owner_id() == client.client_id.unwrap()
                    } else {
                        false
                    };
                    debug!("Created net obj {:?}", prefab);
                    client.net_objects.add(prefab);
                    if is_self {
                        if settings.send_initial_info {
                            client.set_name(&settings.game_username);
                            client.set_color(settings.initial_color);
                            client.set_skin(settings.initial_skin);
                            client.set_hat(settings.initial_hat);
                            client.set_pet(settings.initial_pet);
                        }
                        handler.joined_game(client);
                    }
                }
                GameInfo::ChangeScene { .. } => {
                    if client.is_host() {
                        todo!()
                    }
                }
                _ => warn!("Unhandled game info {:?}", info),
            }
        }
    }

    pub fn request_game_list(&mut self, language: Languages, maps: u8, num_imposters: i8) {
        let req_packet = RequestGameListPacket::new(language, maps, num_imposters);
        self.send_reliable(PacketType::GameList, Box::new(req_packet));
    }

    pub fn disconnect(&mut self) {
        self.should_disconnect = true;
    }

    pub fn send_hello(&mut self, connect_username: &str) {
        self.client.send_hello(Box::new(HelloData {
            version: AMONG_US_VERSION,
            username: connect_username.to_string(),
        }));
    }

    pub fn send_reliable(&mut self, packet_type: PacketType, data: Box<dyn Serialize>) {
        let packet = GenericMessage {
            tag: packet_type as u8,
            data,
        };
        self.client.send_reliable(Box::new(packet));
    }

    pub fn send_unreliable(&mut self, packet_type: PacketType, data: Box<dyn Serialize>) {
        let packet = GenericMessage {
            tag: packet_type as u8,
            data,
        };
        self.client.send_unreliable(Box::new(packet));
    }

    pub fn send_ready(&mut self) {
        let packet = Packet::GameInfo {
            game_id: self.game_id.unwrap(),
            data: vec![GameInfo::ClientReady {
                client_id: self.client_id.unwrap(),
            }],
        };
        self.send_reliable(PacketType::GameInfo, Box::new(packet));
    }

    pub fn join_game_code(&mut self, code: &str) {
        if self.game_id.is_some() {
            unimplemented!();
        }

        let game_id = GameId::from_chars(code);
        self.join_game_id(game_id);
    }

    fn join_game_id(&mut self, game_id: GameId) {
        // Join game
        self.game_id = Some(game_id);
        let join_game_packet = JoinGamePacket {
            game_id,
            maps_owned: 7,
        };
        self.send_reliable(PacketType::GameJoinDisconnect, Box::new(join_game_packet));
    }

    pub fn change_scene(&mut self, scene_name: &str) {
        let packet = Packet::GameInfo {
            game_id: self.game_id.unwrap(),
            data: vec![GameInfo::ChangeScene {
                client_id: self.client_id.unwrap(),
                scene: scene_name.to_string(),
            }],
        };
        self.send_reliable(PacketType::GameInfo, Box::new(packet));
    }

    pub fn set_name(&mut self, name: &str) {
        self.set_player_name(self.client_id.unwrap(), name);
    }

    pub fn set_player_name(&mut self, player_id: i32, name: &str) {
        if self.is_host() {
            todo!()
        } else {
            let control = match self.net_objects.get_player_control(player_id) {
                Some(value) => value,
                None => return,
            };
            let info = control.rpc_check_name(name);
            let packet = Packet::GameInfoTo {
                game_id: self.game_id.unwrap(),
                client_id: self.host_id.unwrap(),
                data: vec![info],
            };
            self.send_reliable(PacketType::GameInfoTo, Box::new(packet));
        }
    }

    pub fn send_chat(&mut self, message: &str) {
        self.send_chat_player(self.client_id.unwrap(), message);
    }

    pub fn send_chat_player(&mut self, player_id: i32, message: &str) {
        if self.is_host() {
            todo!()
        } else {
            let control = match self.net_objects.get_player_control(player_id) {
                Some(value) => value,
                None => return,
            };
            let info = control.rpc_chat_message(message);
            let packet = Packet::GameInfo {
                game_id: self.game_id.unwrap(),
                data: vec![info],
            };
            self.send_reliable(PacketType::GameInfo, Box::new(packet));
        }
    }

    pub fn set_color(&mut self, color_index: u8) {
        self.set_player_color(self.client_id.unwrap(), color_index);
    }

    pub fn set_player_color(&mut self, player_id: i32, color_index: u8) {
        if self.is_host() {
            todo!()
        } else {
            let control = match self.net_objects.get_player_control(player_id) {
                Some(value) => value,
                None => return,
            };
            let info = control.rpc_check_color(color_index);
            let packet = Packet::GameInfoTo {
                game_id: self.game_id.unwrap(),
                client_id: self.host_id.unwrap(),
                data: vec![info],
            };
            self.send_reliable(PacketType::GameInfoTo, Box::new(packet));
        }
    }

    pub fn set_skin(&mut self, skin_index: u32) {
        if self.is_host() {
            todo!()
        } else {
            let control = match self.net_objects.get_player_control(self.client_id.unwrap()) {
                Some(value) => value,
                None => return,
            };
            let info = control.rpc_set_skin(skin_index);
            let packet = Packet::GameInfoTo {
                game_id: self.game_id.unwrap(),
                client_id: self.host_id.unwrap(),
                data: vec![info],
            };
            self.send_reliable(PacketType::GameInfoTo, Box::new(packet));
        }
    }

    pub fn set_hat(&mut self, hat_index: u32) {
        if self.is_host() {
            todo!()
        } else {
            let control = match self.net_objects.get_player_control(self.client_id.unwrap()) {
                Some(value) => value,
                None => return,
            };
            let info = control.rpc_set_hat(hat_index);
            let packet = Packet::GameInfoTo {
                game_id: self.game_id.unwrap(),
                client_id: self.host_id.unwrap(),
                data: vec![info],
            };
            self.send_reliable(PacketType::GameInfoTo, Box::new(packet));
        }
    }

    pub fn set_pet(&mut self, pet_index: u32) {
        if self.is_host() {
            todo!()
        } else {
            let control = match self.net_objects.get_player_control(self.client_id.unwrap()) {
                Some(value) => value,
                None => return,
            };
            let info = control.rpc_set_pet(pet_index);
            let packet = Packet::GameInfoTo {
                game_id: self.game_id.unwrap(),
                client_id: self.host_id.unwrap(),
                data: vec![info],
            };
            self.send_reliable(PacketType::GameInfoTo, Box::new(packet));
        }
    }

    pub fn set_position(&mut self, new_pos: Vector2) {
        self.set_player_position(self.client_id.unwrap(), new_pos);
    }

    pub fn set_player_position(&mut self, player_id: i32, new_pos: Vector2) {
        if self.is_host() {
            todo!()
        } else {
            let transform = match self.net_objects.get_player_transform(player_id) {
                Some(value) => value,
                None => return,
            };
            let info = transform.rpc_snap_to(new_pos);
            let packet = Packet::GameInfoTo {
                game_id: self.game_id.unwrap(),
                client_id: self.host_id.unwrap(),
                data: vec![info],
            };
            self.send_reliable(PacketType::GameInfoTo, Box::new(packet));
        }
    }

    pub fn enter_vent(&mut self, vent_id: u32) {
        self.player_enter_vent(self.client_id.unwrap(), vent_id);
    }

    pub fn player_enter_vent(&mut self, player_id: i32, vent_id: u32) {
        if self.is_host() {
            todo!()
        } else {
            let physics = match self.net_objects.get_player_physics(player_id) {
                Some(value) => value,
                None => return,
            };
            let info = physics.rpc_enter_vent(vent_id);
            let packet = Packet::GameInfo {
                game_id: self.game_id.unwrap(),
                data: vec![info],
            };
            self.send_reliable(PacketType::GameInfo, Box::new(packet));
        }
    }

    pub fn kick_player(&mut self, _player_id: i32, _ban: bool) {
        panic!("Will get you banned from official servers")
        // self.send_reliable(
        //     PacketType::KickPlayer,
        //     Box::new(Packet::KickPlayer {
        //         game_id: self.game_id.unwrap(),
        //         player_id,
        //         ban,
        //     }),
        // );
    }

    pub fn delete_net_object(&mut self, net_id: u32) {
        self.send_reliable(
            PacketType::GameInfo,
            Box::new(Packet::GameInfo {
                game_id: self.game_id.unwrap(),
                data: vec![GameInfo::Destroy { net_id }],
            }),
        )
    }

    pub fn update_game_data(&mut self) {
        let info = self
            .net_objects
            .game_datas
            .get_mut(0)
            .unwrap()
            .rpc_update_player_info();
        self.send_reliable(
            PacketType::GameInfo,
            Box::new(Packet::GameInfo {
                game_id: self.game_id.unwrap(),
                data: vec![info],
            }),
        );
    }
}

#[allow(unused_variables)]
pub trait EventHandler {
    fn disconnect_reason(&mut self, client: &mut Client, reason: DisconnectReason) {}

    fn joined_game(&mut self, client: &mut Client) {}

    fn packet_received(&mut self, client: &mut Client) {}

    fn server_info(&mut self, client: &mut Client, data: ServerListPacket) {}

    fn chat_message(&mut self, client: &mut Client, player_id: i32, message: String) {}
}

pub struct NetObjectHandler {
    pub player_controls: Vec<PlayerControl>,
    pub player_physics: Vec<PlayerPhysics>,
    pub player_transforms: Vec<PlayerTransform>,
    pub worlds: Vec<World>,
    pub lobbies: Vec<Lobby>,
    pub game_datas: Vec<GameData>,
    pub vote_bans: Vec<VoteBanSystem>,
}

impl Default for NetObjectHandler {
    fn default() -> Self {
        Self {
            player_controls: Vec::new(),
            player_physics: Vec::new(),
            player_transforms: Vec::new(),
            worlds: Vec::new(),
            lobbies: Vec::new(),
            game_datas: Vec::new(),
            vote_bans: Vec::new(),
        }
    }
}

impl NetObjectHandler {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get_player_control(&mut self, owner_id: i32) -> Option<&mut PlayerControl> {
        self.player_controls
            .iter_mut()
            .find(|obj| obj.owner_id() == owner_id)
    }

    pub fn get_player_physics(&mut self, owner_id: i32) -> Option<&mut PlayerPhysics> {
        self.player_physics
            .iter_mut()
            .find(|obj| obj.owner_id() == owner_id)
    }

    pub fn get_player_transform(&mut self, owner_id: i32) -> Option<&mut PlayerTransform> {
        self.player_transforms
            .iter_mut()
            .find(|obj| obj.owner_id() == owner_id)
    }

    pub fn add(&mut self, prefab: Prefab) {
        match prefab {
            Prefab::Player(control, physics, transform) => {
                self.player_controls.push(control);
                self.player_physics.push(physics);
                self.player_transforms.push(transform);
            }
            Prefab::World(world) => self.worlds.push(world),
            Prefab::Lobby(lobby) => self.lobbies.push(lobby),
            Prefab::GameData(game_data, vote_ban) => {
                self.game_datas.push(game_data);
                self.vote_bans.push(vote_ban);
            }
            Prefab::Unknown => warn!("Tried to add unknown prefab to handler"),
        }
    }

    /// Remove an object, returning true if object exists
    pub fn remove(&mut self, net_id: u32) -> bool {
        if let Some(index) = self
            .player_controls
            .iter()
            .position(|obj| obj.net_id() == net_id)
        {
            self.player_controls.remove(index);
            return true;
        }
        if let Some(index) = self
            .player_physics
            .iter()
            .position(|obj| obj.net_id() == net_id)
        {
            self.player_physics.remove(index);
            return true;
        }
        if let Some(index) = self
            .player_transforms
            .iter()
            .position(|obj| obj.net_id() == net_id)
        {
            self.player_transforms.remove(index);
            return true;
        }
        if let Some(index) = self.worlds.iter().position(|obj| obj.net_id() == net_id) {
            self.worlds.remove(index);
            return true;
        }
        if let Some(index) = self.lobbies.iter().position(|obj| obj.net_id() == net_id) {
            self.lobbies.remove(index);
            return true;
        }
        if let Some(index) = self
            .game_datas
            .iter()
            .position(|obj| obj.net_id() == net_id)
        {
            self.game_datas.remove(index);
            return true;
        }
        if let Some(index) = self.vote_bans.iter().position(|obj| obj.net_id() == net_id) {
            self.vote_bans.remove(index);
            return true;
        }
        false
    }

    pub fn get(&mut self, net_id: u32) -> Option<&mut dyn NetObject> {
        if let Some(obj) = self
            .player_controls
            .iter_mut()
            .find(|obj| obj.net_id() == net_id)
        {
            return Some(obj);
        }
        if let Some(obj) = self
            .player_physics
            .iter_mut()
            .find(|obj| obj.net_id() == net_id)
        {
            return Some(obj);
        }
        if let Some(obj) = self
            .player_transforms
            .iter_mut()
            .find(|obj| obj.net_id() == net_id)
        {
            return Some(obj);
        }
        if let Some(obj) = self.worlds.iter_mut().find(|obj| obj.net_id() == net_id) {
            return Some(obj);
        }
        if let Some(obj) = self.lobbies.iter_mut().find(|obj| obj.net_id() == net_id) {
            return Some(obj);
        }
        if let Some(obj) = self
            .game_datas
            .iter_mut()
            .find(|obj| obj.net_id() == net_id)
        {
            return Some(obj);
        }
        if let Some(obj) = self.vote_bans.iter_mut().find(|obj| obj.net_id() == net_id) {
            return Some(obj);
        }
        None
    }
}

#[derive(Debug)]
struct HelloData {
    version: u32,
    username: String,
}

impl Serialize for HelloData {
    fn serialize(&self, w: &mut PacketWriter) {
        w.write_u8(0); // Reserved byte?
        w.write_u32(self.version);
        w.write_string(&self.username);
    }
}
