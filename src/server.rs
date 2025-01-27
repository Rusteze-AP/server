mod commands_handler;
mod logger_settings;
mod packet_dispatcher;
mod utils;
mod video_chunker;

use crate::database::Database;

use crossbeam::channel::{select_biased, Receiver, Sender};
use logger::{LogLevel, Logger};
use packet_forge::{PacketForge, SessionIdT};
use routing_handler::RoutingHandler;
use std::collections::{HashMap, HashSet};
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
    recv_fragments_map: HashMap<(NodeId, SessionIdT), Vec<Fragment>>, // (client_id, session_id) -> fragment --- *Keep track of the received fragments*
    // Handle outgoing packets
    sent_fragments_history: HashMap<(u64, SessionIdT), Packet>, // (fragment_index, session_id) -> Packet(Fragment) --- *Save the sent fragments*
    // Storage data structures
    database: Database,
    // Network graph
    routing_handler: RoutingHandler,
    curr_flood_id: u64,
    used_flood_id: HashSet<u64>,
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
            recv_fragments_map: HashMap::new(),
            sent_fragments_history: HashMap::new(),
            database: Database::new(&format!("db/server-{id}"), id),
            routing_handler: RoutingHandler::new(),
            curr_flood_id: 0,
            used_flood_id: HashSet::new(),
            logger: Logger::new(LogLevel::None as u8, false, format!("SERVER-{id}")),
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
            self.logger.log_error(&msg);
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
                        self.logger.log_error("Error receiving command!");
                        break;
                    }
                },
                recv(self.packet_recv) -> packet => {
                    if let Ok(packet) = packet {
                        self.packet_dispatcher(&packet);
                    } else {
                        self.logger.log_error("Error receiving message!");
                        break;
                    }
                }
            );
        }
    }
}
