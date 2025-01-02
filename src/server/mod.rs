mod commands_handler;
mod logger_settings;
mod messages_handler;
mod utils;

use crossbeam::channel::{select_biased, Receiver, Sender};
use logger::{LogLevel, Logger};
use packet_forge::{ClientType, FileHash, FileMetadata, PacketForge};
use std::collections::{HashMap, HashSet};
use wg_internal::controller::{DroneCommand, DroneEvent};
use wg_internal::network::NodeId;
use wg_internal::packet::{Fragment, Packet};

#[derive(Debug, Clone)]
pub struct ClientInfo {
    pub client_type: ClientType,         // "audio" or "video"
    pub shared_files: HashSet<FileHash>, // Files shared by this client
}

#[derive(Debug, Clone)]
pub struct FileEntry {
    pub file_metadata: FileMetadata,
    pub peers: HashSet<NodeId>, // List of clients sharing the file
}

pub struct Server {
    id: NodeId,
    controller_send: Sender<DroneEvent>,
    controller_recv: Receiver<DroneCommand>,
    packet_recv: Receiver<Packet>,
    senders: HashMap<NodeId, Sender<Packet>>,
    packet_forge: PacketForge,
    packets_map: HashMap<(NodeId, u64), Vec<Fragment>>, // (client_id, session_id) -> fragment
    terminated: bool,
    // Storage data structures
    clients: HashMap<NodeId, ClientInfo>,
    files: HashMap<FileHash, FileEntry>,
    // Logger
    logger: Logger,
}

impl Server {
    #[must_use]
    pub fn new(
        id: NodeId,
        command_send: Sender<DroneEvent>,
        command_recv: Receiver<DroneCommand>,
        receiver: Receiver<Packet>,
        senders: HashMap<NodeId, Sender<Packet>>,
    ) -> Self {
        Server {
            id,
            controller_send: command_send,
            controller_recv: command_recv,
            packet_recv: receiver,
            senders,
            packet_forge: PacketForge::new(),
            packets_map: HashMap::new(),
            terminated: false,
            clients: HashMap::new(),
            files: HashMap::new(),
            logger: Logger::new(LogLevel::None as u8, false, "Server-tracker".to_string()),
        }
    }

    #[must_use]
    pub fn get_id(&self) -> NodeId {
        self.id
    }

    pub fn run(&mut self) {
        loop {
            if self.terminated {
                break;
            }

            select_biased!(
                recv(self.controller_recv) -> command => {
                    if let Ok(command) = command {
                        self.command_dispatcher(&command);
                    } else {
                        self.logger.log_error(format!("Error receiving command for server {}", self.id).as_str());
                        break;
                    }
                },
                recv(self.packet_recv) -> packet => {
                    if let Ok(packet) = packet {
                        self.packet_dispatcher(&packet);
                    } else {
                        self.logger.log_error(format!("Error receiving message for server {}", self.id).as_str());
                        break;
                    }
                }
            );
        }
    }
}
