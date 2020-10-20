
# Among Us Protocol

An incomplete dissection of the Among Us protocol

Last updated using Among Us v2020.9.22

Code in parts where I'm too lazy to explain properly

## Overview

Among Us communicates using a version of [RUDP](https://en.wikipedia.org/wiki/Reliable_User_Datagram_Protocol) over ports 22023 (game) and 22024 (announcements). The library they use is called Hazel. An older version of it's source is on [Github](https://github.com/willardf/Hazel-Networking). Documentation from an even older version is hosted at <https://www.darkriftnetworking.com/Hazel/Docs/html/>.

## Data Types

All types are little endian unless specified otherwise

### Basic

|Name|Size (bytes)|Encodes|Comments|
|--|--|--|--|
|bool|1|A boolean value|`0x01` for true, `0x00` for false|
|i8|1|A signed 8 bit integer||
|i16|2|A signed 16 bit integer||
|i32|4|A signed 32 bit integer||
|u8|1|A unsigned 8 bit integer||
|u16|2|A unsigned 16 bit integer||
|u32|4|A unsigned 32 bit integer||
|packed_u32|1+|An unsigned 32-bit integer|Encoded using [LEB128](https://en.wikipedia.org/wiki/LEB128). Also see [BinaryReader.Read7BitEncodedInt](https://docs.microsoft.com/en-us/dotnet/api/system.io.binaryreader.read7bitencodedint?view=netcore-3.1)|
|packed_i32|1+|An signed 32-bit integer|See above
|String|Varies|A UTF-8 string|Prefixed by it's size as a `packed_u32`|
|Array of *X*|Array length times size of `X`|Zero or more of `X` stored sequentially without breaks||
|Array[C] of `X`|C times size of `X`|C amount of `X` stored sequentially without breaks||
|Pair[A, B]|size of `A` + size of `B`|A pair of items with type `A` and type `B` respectively||

### Message

Messages compose much of the protocol, with the base packet being a message, and then further messages usually nested within.

|Field Name|Type|Comments|
|--|--|--|
|data_length|u16|Length of `data`|
|tag|u8||
|data|Array of u8||

### Game ID

Game IDs are stored and sent as `i32`s, the tricky part is parsing and printing them to and from strings. There are two different versions. V1 with 4 char codes and V2 with 6 char codes.

See [here](/common/src/data/objects.rs#L32) for reference code

### Address

|Field Name|Type|Comments|
|--|--|--|
|ip|Array[4] of u8|The 4 bytes of an IPV4 address|
|port|u16||

### Vector2

A slightly more compact encoding of a XY Vector

|Field Name|Type|Comments|
|--|--|--|
|v|u16||
|v2|u16||

```rust
    fn deserialize<T: PacketRead + Read>(r: &mut PacketReader<T>) -> io::Result<Vector2> {
        let v = r.read_u16()? as f32 / 65535.;
        let v2 = r.read_u16()? as f32 / 65535.;
        let x = (v.max(0.).min(1.) * 80.) - 40.;
        let y = (v2.max(0.).min(1.) * 80.) - 40.;
        Ok(Vector2 { x, y })
    }
```

### Hazel Packets

These are the basic packet types used by Hazel. The first byte corresponds to the packet type

|Value|Type|
|--|--|
|`0x00`|Unreliable|
|`0x01`|Reliable|
|`0x08`|Hello|
|`0x09`|Disconnect|
|`0x0a`|Acknowledge|
|`0x0c`|Keep-Alive|

#### Unreliable

A send-and-forget packet

|Field|Type|Comments|
|--|--|--|
|type|u8|`0x00`|
|data|Array of u8|Rest of packet|

#### Reliable

A packet that is re-sent periodically until acknowledged by the receiver

|Field|Type|Comments|
|--|--|--|
|type|u8|`0x01`|
|ack_id|Big Endian u16|Packet ID|
|data|Array of u8|Rest of packet|

#### Hello

Used to initialize a connection

|Field|Type|Comments|
|--|--|--|
|type|u8|`0x01`|
|ack_id|Big Endian u16|Packet ID|
|data|Array of u8|Rest of packet|

#### Disconnect

Used to close a connection

|Field|Type|Comments|
|--|--|--|
|type|u8|`0x09`|

#### Acknowledge

Sent to acknowledge a Reliable, Hello or Keep-Alive packet

|Field|Type|Comments|
|--|--|--|
|type|u8|`0x0a`|
|ack_id|Big Endian u16|ID of the packet to acknowledge|
|ack_id_flags|u8|Extra packets to acknowledge. The index of each set bit is the offset of the packet being acknowledged calculated by `ack_id - index - 1`|

#### Keep-Alive

Sent periodically to keep the connection alive

|Field|Type|Comments|
|--|--|--|
|type|u8|`0x0c`|
|ack_id|Big Endian u16|Packet ID|

## Connecting

To connect to a server a [Hello](#Hello) packet is sent with the following data

|Field|Type|Comments|
|--|--|--|
|reserved|u8|`0x00`|
|version|u32|`50_51_65_50` as of v2020.9.22|
|username|String|Up to 12 characters. Some characters are restricted|

## Among Us Messages

The messages that can be sent and received once connected. Clients and servers mostly send the same packet types and tags, though react differently. Sent using either [Reliable](#Reliable) or [Unreliable](#Unreliable) packets. Each is encoded as a [Message](#Message) with the `tag` corresponding to the type. Multiple messages can be sent in a single packet

|Tag|Type|
|--|--|
|`0x00`|HostingGame|
|`0x01`|GameJoinDisconnect|
|`0x02`|GameStarted|
|`0x04`|PlayerLeft|
|`0x05`|GameInfo|
|`0x06`|GameInfoTo|
|`0x07`|JoinedGame|
|`0x0a`|AlterGameInfo|
|`0x0b`|KickPlayer|
|`0x0d`|ChangeServer|
|`0x0e`|ServerList|
|`0x10`|GameList|

### 0x00 - HostingGame

Sent by the server to tell the client that it is now hosting a (new?) game

|Field|Type|Comments|
|--|--|--|
|game_id|GameId|id of the game|

### 0x01 - GameJoinDisconnect

#### Sent by Client

Sent to join a game

|Field|Type|Comments|
|--|--|--|
|game_id|GameId|ID of the game to join|
|maps_owned|u8|Bit flags that specify which maps the client owns. Currently always `0x07` (`0b111`) as all three maps are free

#### Sent by Server

The type of packet is different depending on the first `i32`

##### < -2 || > 255

Joined game

|Field|Type|Comments|
|--|--|--|
|game_id|GameId|ID of joined game|
|player_id|i32|Player ID of the client|
|host_id|i32| Player ID of the host|

##### -1 - 255

Disconnected

|Field|Type|Comments|
|--|--|--|
|reason|i32|`DisconnectReason` index|
|custom_reason|String|Only if `reason == 8`

```rust
match reason {
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
}
```

### 0x02 - GameStarted

Sent to tell the client the game has started. No extra data

### 0x04 - PlayerLeft

|Field|Type|Comments|
|--|--|--|
|game_id|GameId|ID of joined game|
|player_id|i32|Leaving player's ID|
|host_id|i32| Player ID of the possibly new host|
|reason|u8|Only read if there is extra data in the packet|

```csharp
public enum DisconnectReasons
{
    ExitGame,
    GameFull,
    GameStarted,
    GameNotFound,
    IncorrectVersion = 5,
    Banned,
    Kicked,
    Custom,
    Destroy = 16,
    Error,
    IncorrectGame,
    ServerRequest,
    ServerFull,
    IntentionalLeaving = 208,
    FocusLostBackground = 207,
    FocusLost = 209,
    NewConnection
}
```

### 0x05 - GameInfo

When sent to the server broadcasts data to every player in the game

|Field|Type|Comments|
|--|--|--|
|game_id|GameId|Game ID the data is for|
|data|-|See [Game Info](#Game%20Info)|

### 0x06 - GameInfoTo

Same as GameInfo but to a specific player

|Field|Type|Comments|
|--|--|--|
|game_id|GameId|Game ID the data is for|
|client_id|i32|ID of the player to send the data to|
|data|-|See [Game Info](#Game%20Info)|

### 0x07 - JoinedGame

Packet sent to the client upon joining a game

|Field|Type|Comments|
|--|--|--|
|game_id|GameId||
|client_id|i32||
|host_id|i32||
|player_ids_count|packed_u32|Size of `player_ids`|
|player_ids|Array of packed_i32||

### 0x0a - AlterGameInfo

Packet sent to the client upon joining a game

|Field|Type|Comments|
|--|--|--|
|game_id|GameId||
|setting_to_alter|u8|Always `0x01`|
|is_public|bool|True if game is public, false otherwise|

### 0x0b - KickPlayer

Warning: **Official servers will ban you for sending this**

Request a player is kicked/banned

|Field|Type|Comments|
|--|--|--|
|game_id|GameId||
|player_id|i32||
|ban|bool|True if ban, false if kick|

### 0x0d - ChangeServer

Sent to the client to get it to change to a new server

|Field|Type|Comments|
|--|--|--|
|ip|Array[4] of u8|IP address|
|port|u16||

### 0x0e - ServerList

Sent to the client to tell it which master servers are available

|Field|Type|Comments|
|--|--|--|
|name|String||
|ip|Array[4] of u8|Yes, no IPV6|
|port|u16||
|connection_failures|packed_u32|?|

#### Server

|Field|Type|Comments|
|--|--|--|
|server_count|packed_u32|Size of `servers`|
|server_messages|Array[`server_count`] of Message[Server]||

### 0x10 - GameList

Sent to the client to tell it which master servers are available

|Field|Type|Comments|
|--|--|--|
|game_list_messages|Message[GameListings]|Outer message. Tag should be `0`|

#### GameListings

|Field|Type|Comments|
|--|--|--|
|game_list_message|Array of Message[GameListing]|Read until end|

#### GameListing

|Field|Type|Comments|
|--|--|--|
|address|Address|Server game is being hosted on|
|id|GameID||
|host_username|String||
|player_count|u8||
|age|packed_u32|Something to do with the time the game has been running|
|map_id|u8|Skeld `0x00`, Porus `0x01`, Mira `0x02`|
|num_imposters|u8|Number of imposters|
|max_players|u8||

## Game Info

### Data structure

|Field|Type|Comments|
|--|--|--|
|messages|Array of Message[GameInfo]|Read until end. Tag corresponds to type|

|Tag|Type|
|--|--|
|`0x01`|UpdateData|
|`0x02`|RPC|
|`0x04`|CreateFromPrefab|
|`0x05`|Destroy|
|`0x06`|ChangeScene|
|`0x07`|ClientReady|

### 0x01 - UpdateData

Updates the data of an existing net object with id `net_id`. Ignored if object doesn't exist

|Field|Type|Comments|
|--|--|--|
|net_id|packed_u32||
|data|-|Rest of message|

### 0x02 - RPC

Calls an RPC of an existing net object with id `net_id`. Ignored if object doesn't exist

|Field|Type|Comments|
|--|--|--|
|net_id|packed_u32||
|call_id|u8|RPC id|
|data|-|Rest of message|

### 0x04 - CreateFromPrefab

Creates new net objects. In the actual client this uses Unity prefabs.

|Field|Type|Comments|
|--|--|--|
|prefab_id|packed_u32|The type of prefab to create|
|owner_id|packed_i32|Owner of the net object. Stored but never checked|
|spawn_flags|u8|Set to 1 to create the client's character graphically. Can be ignored|
|num_children|packed_u32|Should correspond to the number of net objects the selected prefab creates|
|children_messages|Array[num_children] of InitData||

|Prefab ID|Name|`num_children`|Net Objects/Children|
|--|--|--|--|
|`0x00`|World|`1`|World|
|`0x01`|MeetingHub|?|?|
|`0x02`|Lobby|`1`|Lobby|
|`0x03`|GameData|`2`|GameData, VoteBanSystem|
|`0x04`|Player|`3`|PlayerControl, PlayerPhysics, PlayerTransform|
|`0x05`|HeadQuarters|?|?|

#### Init Data

|Field|Type|Comments|
|--|--|--|
|net_id|packed_u32|The id of the new net object|
|message|Message|Tag always `1`. Data is used to initialize a net object (See below)|

### 0x05 - Destroy

Deletes a net object

|Field|Type|Comments|
|--|--|--|
|net_id|packed_u32||

### 0x06 - ChangeScene

Changes a players scene

|Field|Type|Comments|
|--|--|--|
|client_id|packed_i32||
|scene|String|Either `"OnlineGame"` or `"Tutorial"`|

### 0x07 - ClientReady

Sets a player as ready. Needed to start a game

|Field|Type|Comments|
|--|--|--|
|client_id|packed_i32||

## Net Objects

`Init` is the structure used to create a new net object. Data from [Init Data](#Init%20Data)

`Update` is the structure used to update an existing net object

`RPC` the structure used to call remote procedures. **Only partially complete**

### World

Used to control the map, including doors, visible task animations, sabotages etc.

#### World Init

[Implementation @ `common/src/data/netobject.rs#L386`](common/src/data/netobject.rs#L386)

|Field|Type|Comments|
|--|--|--|
|reactor_countdown|f32||
|ucp_count|packed_u32||
|user_console_pairs|Array[`ucp_count`] of Pair[u8, u8]||
|expected_switches|u8||
|actual_switches|u8||
|elec_value|u8||
|life_supp_countdown|f32||
|comp_cons_count|packed_u32||
|completed_consoles|Array[`comp_cons_count`] of packed_u32||
|med_user_list_count|||
|med_user_list|Array[`med_user_list_count`] of i8||
|camera_in_use|bool||
|comms_active|bool||
|door_open|Array[`13`] of bool||
|sabotage_timer|f32||

#### World Update

Essentially the same as Init but with each section being optional, indicated by a packed_u32 bitflag

[Implementation @ `common/src/data/netobject.rs#L437`](common/src/data/netobject.rs#L437)

..

#### World RPC

##### 0x00 - Close Doors

|Field|Type|Comments|
|--|--|--|
|door_id|u8||

##### 0x01 - Repair System

|Field|Type|Comments|
|--|--|--|
|system_type|u8||
|player_net_id|packed_u32|Net id of the player's Player Control|
|amount|u8||

### Lobby

#### Lobby Init

Data should be empty

#### Lobby Update

Data should be empty

#### Lobby RPC

Doesn't have any

### GameData

#### GameData Init

[Implementation @ `common/src/data/netobject.rs#L549`](common/src/data/netobject.rs#L549)

|Field|Type|Comments|
|--|--|--|
|player_count|packed_u32||
|players|Array[`player_count`] of Pair[u8, PlayerData]||

##### PlayerData

[Implementation @ `common/src/data/objects.rs#L334`](common/src/data/objects.rs#L334)

..

#### GameData Update

[Implementation @ `common/src/data/netobject.rs#L590`](common/src/data/netobject.rs#L590)

..

#### GameData RPC

[Implementation @ `common/src/data/netobject.rs#L598`](common/src/data/netobject.rs#L598)

..

### VoteBanSystem

..

### PlayerControl

#### PlayerControl Init

[Implementation @ `common/src/data/netobject.rs#L59`](common/src/data/netobject.rs#L59)

|Field|Type|Comments|
|--|--|--|
|is_new|bool||
|player_id|u8||

#### PlayerControl Update

[Implementation @ `common/src/data/netobject.rs#L170`](common/src/data/netobject.rs#L170)

|Field|Type|Comments|
|--|--|--|
|player_id|u8||

#### PlayerControl RPC

[Implementation @ `common/src/data/netobject.rs#L175`](common/src/data/netobject.rs#L175)

```rust
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
```

..

### PlayerPhysics

#### PlayerPhysics Init

Empty

#### PlayerPhysics Update

Empty

#### PlayerPhysics RPC

[Implementation @ `common/src/data/netobject.rs#L252`](common/src/data/netobject.rs#L252)

##### 0x13 - EnterVent

|Field|Type|Comments|
|--|--|--|
|vent_id|packed_u32||

##### 0x14 - ExitVent

|Field|Type|Comments|
|--|--|--|
|vent_id|packed_u32||

### PlayerTranform

Controls players position, movement

#### PlayerTransform Init

[Implementation @ `common/src/data/netobject.rs#L294`](common/src/data/netobject.rs#L294)

|Field|Type|Comments|
|--|--|--|
|last_seq_id|u16||
|target_position|Vector2||
|velocity|Vector2||

#### PlayerTransform Update

[Implementation @ `common/src/data/netobject.rs#L323`](common/src/data/netobject.rs#L323)

|Field|Type|Comments|
|--|--|--|
|last_seq_id|u16||
|target_position|Vector2||
|velocity|Vector2||

#### PlayerTransform RPC

##### 0x15 - SnapTo

Sets velocity to zero

|Field|Type|Comments|
|--|--|--|
|new_position|Vector2||
|new_seq_id|u16||

..
