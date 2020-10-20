use std::{
    collections::HashMap,
    io::Result,
    net::{SocketAddr, UdpSocket},
    sync::{
        mpsc::{channel, Receiver, RecvTimeoutError, Sender},
        Arc, RwLock,
    },
    time::{Duration, Instant},
};

use common::{
    data::{HazelPacket, HazelPacketOut},
    reader::{IntoReader, Serialize},
};

use log::{error, info};

pub const DEFAULT_PORT: u16 = 22023;
pub const _ANNOUNCE_PORT: u16 = 22024;
const BUFFER_SIZE: usize = 65_507;

/// The main servers Among Us connects to
pub enum MainServer {
    Europe,
    NorthAmerica,
    Asia,
}

impl MainServer {
    /// Get the address of a server
    pub fn to_addr(&self) -> SocketAddr {
        match self {
            MainServer::Europe => SocketAddr::from(([172, 105, 251, 170], DEFAULT_PORT)),
            MainServer::NorthAmerica => SocketAddr::from(([66, 175, 220, 120], DEFAULT_PORT)),
            MainServer::Asia => SocketAddr::from(([139, 162, 111, 196], DEFAULT_PORT)),
        }
    }
}

/// UDP client that implements the Hazel protocol
///
/// Sends a disconnect packet when dropped
pub struct NetClient {
    /// The `Sender` for the packet sending channel
    packet_out_send: Sender<HazelPacketOut>,
    packet_in_recv: Receiver<HazelPacket>,
    ack_handler: Arc<RwLock<AckHandler>>,
}

// TODO: Track received packets for missed ones
impl NetClient {
    /// Creates a client and binds it to a random local port, then connects to the
    /// given address and starts the send/receive loops
    pub fn connect_direct(addr: SocketAddr) -> Result<Self> {
        // Bind udp socket
        let any_address = SocketAddr::from(([0, 0, 0, 0], 0));
        let socket = UdpSocket::bind(any_address)?;

        // Connect to remote server
        socket.connect(addr)?;
        info!("Connected to {}", addr);

        let (packet_out_send, packet_out_recv) = channel::<HazelPacketOut>();
        let (packet_in_send, packet_in_recv) = channel::<HazelPacket>();
        let ack_handler = AckHandler {
            ack_index: 1,
            unconfirmed: HashMap::new(),
        };
        let ack_handler = Arc::new(RwLock::new(ack_handler));

        // Send thread
        let send_socket = socket.try_clone().unwrap();
        let _send_thread = {
            let ack_handler = ack_handler.clone();
            std::thread::spawn(move || loop {
                let packet = packet_out_recv.recv_timeout(Duration::from_millis(50));
                match packet {
                    Ok(packet) => {
                        let packet_bytes = packet.serialize_bytes();

                        // Send packet
                        send_socket.send(&packet_bytes).unwrap();

                        // Handle ack stuff
                        {
                            let mut ack_handler = ack_handler.write().unwrap();
                            match packet {
                                HazelPacketOut::Unreliable { .. } => (),
                                HazelPacketOut::Reliable { ack_id, .. } => {
                                    // Add to unconfirmed, checking not already inserted
                                    assert!(ack_handler
                                        .unconfirmed
                                        .insert(ack_id, (Instant::now(), packet_bytes.clone()))
                                        .is_none());
                                }
                                HazelPacketOut::Disconnect => (),
                                HazelPacketOut::Hello { ack_id, .. } => {
                                    // Add to unconfirmed, checking not already inserted
                                    assert!(ack_handler
                                        .unconfirmed
                                        .insert(ack_id, (Instant::now(), packet_bytes.clone()))
                                        .is_none());
                                }
                                HazelPacketOut::Acknowledge { .. } => (),
                                HazelPacketOut::KeepAlive { ack_id } => {
                                    // Add to unconfirmed, checking not already inserted
                                    assert!(ack_handler
                                        .unconfirmed
                                        .insert(ack_id, (Instant::now(), packet_bytes.clone()))
                                        .is_none());
                                }
                            }
                        }
                    }
                    Err(RecvTimeoutError::Timeout) => (),
                    Err(RecvTimeoutError::Disconnected) => break,
                }

                // Resend unacknowledged packets
                {
                    // Lock ack handler
                    let mut ack_handler = ack_handler.write().unwrap();

                    // Temporarily take unconfirmed
                    let unconfirmed =
                        std::mem::replace(&mut ack_handler.unconfirmed, HashMap::new());

                    // Partition by time since last send
                    let (to_repeat_send, keep) = unconfirmed.into_iter().partition::<HashMap<
                        u16,
                        (Instant, Vec<u8>),
                    >, _>(
                        |(_, (instant, _))| instant.elapsed() >= Duration::from_millis(1000),
                    );

                    // Replace unconfirmed
                    ack_handler.unconfirmed = keep;

                    // Repeat
                    to_repeat_send.into_iter().for_each(|(_, (_, data))| {
                        send_socket.send(&data).unwrap();
                    });
                }
            })
        };

        // Receive thread
        let recv_socket = socket.try_clone().unwrap();
        let _recv_thread = {
            let packet_out_send = packet_out_send.clone();
            let ack_handler = ack_handler.clone();
            std::thread::spawn(move || loop {
                // Receive packet
                let mut buffer = vec![0; BUFFER_SIZE];
                match recv_socket.recv(&mut buffer) {
                    Ok(size) => buffer.resize(size, 0),
                    Err(error) => {
                        error!("{} {:?}", error, error.kind());
                        break;
                    }
                }

                // Read packet
                let mut r = buffer.into_reader();
                let packet = r.read::<HazelPacket>();
                if let Err(packet_error) = packet {
                    error!("Error reading hazel packet {}", packet_error);
                    continue;
                }
                let packet = packet.unwrap();

                // Handle packet
                {
                    let mut ack_handler = ack_handler.write().unwrap();
                    match packet {
                        HazelPacket::Unreliable { .. } => (),
                        HazelPacket::Reliable { ack_id, .. } => {
                            packet_out_send
                                .send(HazelPacketOut::Acknowledge { ack_id })
                                .unwrap();
                        }
                        HazelPacket::Disconnect => (),
                        HazelPacket::Hello { ack_id, .. } => {
                            packet_out_send
                                .send(HazelPacketOut::Acknowledge { ack_id })
                                .unwrap();
                        }
                        HazelPacket::Acknowledge { ack_id } => {
                            ack_handler.unconfirmed.remove(&ack_id);
                        }
                        HazelPacket::KeepAlive { ack_id } => {
                            packet_out_send
                                .send(HazelPacketOut::Acknowledge { ack_id })
                                .unwrap();
                        }
                    }
                }

                // Send packet upwards
                if packet_in_send.send(packet).is_err() {
                    // Exit if channel closed
                    return;
                }
            })
        };

        // Return client
        let client = NetClient {
            packet_out_send,
            packet_in_recv,
            ack_handler,
        };
        Ok(client)
    }

    /// Creates a client and binds it to a random local port, then connects to the
    /// given server and starts the send/receive loops
    pub fn connect(server: MainServer) -> Result<Self> {
        Self::connect_direct(server.to_addr())
    }

    /// Sends a packet to the send thread
    fn send(&self, packet: HazelPacketOut) {
        self.packet_out_send.send(packet).unwrap();
    }

    /// Read a packet
    pub fn read_packet(&self) -> HazelPacket {
        self.packet_in_recv.recv().unwrap()
    }

    pub fn send_unreliable(&mut self, data: Box<dyn Serialize>) {
        self.send(HazelPacketOut::Unreliable { data });
    }

    pub fn send_reliable(&mut self, data: Box<dyn Serialize>) {
        let ack_id = self.ack_handler.write().unwrap().get_next_index();
        self.send(HazelPacketOut::Reliable { ack_id, data });
    }

    /// Tells the server to initialize the connection
    /// Optionally send extra data unrelated to the Hazel protocol
    pub fn send_hello(&mut self, data: Box<dyn Serialize>) {
        let ack_id = self.ack_handler.write().unwrap().get_next_index();
        self.send(HazelPacketOut::Hello { ack_id, data });
    }

    /// Sends a disconnect packet
    fn send_disconnect(&mut self) {
        self.send(HazelPacketOut::Disconnect);
    }
}

impl Drop for NetClient {
    fn drop(&mut self) {
        self.send_disconnect();
    }
}

/// Helper struct mainly for thread sync
struct AckHandler {
    ack_index: u16,
    unconfirmed: HashMap<u16, (Instant, Vec<u8>)>,
}

impl AckHandler {
    fn get_next_index(&mut self) -> u16 {
        let value = self.ack_index;
        self.ack_index = self.ack_index.wrapping_add(1);
        value
    }
}
