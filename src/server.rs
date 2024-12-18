use crossbeam::channel::{Receiver, Sender, TryRecvError};
use packet_forge::{PacketForge, TextMessage};
use std::{collections::HashMap, thread, time::Duration};
use wg_internal::controller::{DroneCommand, DroneEvent};
use wg_internal::network::NodeId;
use wg_internal::packet::{Fragment, Packet, PacketType};

pub struct Server {
    id: NodeId,
    command_send: Sender<DroneEvent>,
    command_recv: Receiver<DroneCommand>,
    receiver: Receiver<Packet>,
    senders: HashMap<NodeId, Sender<Packet>>,
    packet_forge: PacketForge,
    packets_map: HashMap<u64, Vec<Fragment>>,
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
            command_send,
            command_recv,
            receiver,
            senders,
            packet_forge: PacketForge::new(),
            packets_map: HashMap::new(),
        }
    }

    pub fn get_id(&self) -> NodeId {
        self.id
    }

    fn handle_packets(&mut self, packet: Packet) {
        let session_id = packet.session_id;
        match packet.pack_type {
            PacketType::MsgFragment(frag) => {
                self.packets_map.entry(session_id).or_default().push(frag);

                let fragments = self.packets_map.get(&session_id).unwrap();
                let total_fragments = fragments[0].total_n_fragments;
                if fragments.len() as u64 == total_fragments {
                    let res = self.packet_forge.assemble::<TextMessage>(fragments.clone());
                    match res {
                        Ok(msg) => {
                            println!("Server {} received a text message: {:?}", self.id, msg);
                        }
                        Err(err) => {
                            eprintln!("Error parsing message for server {}: {:?}", self.id, err);
                        }
                    }
                    self.packets_map.remove(&session_id);
                }
            }
            _ => {
                println!("Server {} received a packet: {:?}", self.id, packet);
            }
        }
    }

    pub fn run(&mut self) {
        loop {
            thread::sleep(Duration::from_secs(1));

            // Check if there's a message from the drone
            match self.receiver.try_recv() {
                Ok(packet) => self.handle_packets(packet),
                Err(TryRecvError::Empty) => {
                    println!("No messages for server {}", self.id);
                }
                Err(err) => {
                    eprintln!("Error receiving message for server {}: {:?}", self.id, err);
                }
            }
        }
    }
}
