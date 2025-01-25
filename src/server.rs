mod commands_handler;
mod logger_settings;
mod packet_dispatcher;
mod utils;
mod video_chunker;

use crate::database::Database;

use crossbeam::channel::{select_biased, Receiver, Sender};
use logger::{LogLevel, Logger};
use packet_forge::{ClientType, FileHash, Metadata, PacketForge, SessionIdT};
use routing_handler::RoutingHandler;
use std::collections::{HashMap, HashSet};
use std::fmt::Debug;
use wg_internal::controller::{DroneCommand, DroneEvent};
use wg_internal::network::NodeId;
use wg_internal::packet::{Fragment, Packet};

pub struct Server {
    id: NodeId,
    controller_send: Sender<DroneEvent>,
    controller_recv: Receiver<DroneCommand>,
    packet_recv: Receiver<Packet>,
    packet_send: HashMap<NodeId, Sender<Packet>>,
    terminated: bool,
    // Handle incoming packages
    packet_forge: PacketForge,
    packets_map: HashMap<(NodeId, SessionIdT), Vec<Fragment>>, // (client_id, session_id) -> fragment
    // Handle outgoing packets
    packets_history: HashMap<(u64, SessionIdT), Packet>, // (fragment_index, session_id) -> Packet
    // Storage data structures
    database: Database,
    // Network graph
    routing_handler: RoutingHandler,
    flood_id: u64,
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
            packet_send: senders,
            terminated: false,
            packet_forge: PacketForge::new(),
            packets_map: HashMap::new(),
            packets_history: HashMap::new(),
            database: Database::new(&format!("db/server-{}", id), id),
            routing_handler: RoutingHandler::new(),
            flood_id: 0,
            logger: Logger::new(LogLevel::None as u8, false, "Server".to_string()),
        }
    }

    #[must_use]
    pub fn get_id(&self) -> NodeId {
        self.id
    }

    pub fn run(&mut self, db_path: &str) {
        // At start perform the first flood_request
        self.init_flood_request();

        // Init database
        let res = self
            .database
            .init(db_path, Some("init_songs.json"), Some("init_videos.json"));

        if let Err(msg) = res {
            self.log_error(&msg);
            return;
        }

        loop {
            if self.terminated {
                break;
            }

            select_biased!(
                recv(self.controller_recv) -> command => {
                    if let Ok(command) = command {
                        self.command_dispatcher(&command);
                    } else {
                        self.log_error("Error receiving command!");
                        break;
                    }
                },
                recv(self.packet_recv) -> packet => {
                    if let Ok(packet) = packet {
                        self.packet_dispatcher(&packet);
                    } else {
                        self.log_error("Error receiving message!");
                        break;
                    }
                }
            );
        }
    }
}
