use crossbeam::channel::{Receiver, Sender};
use packet_forge::{PacketForge, TextMessage};
use std::{collections::HashMap, thread, time::Duration};
use wg_internal::controller::{DroneCommand, DroneEvent};
use wg_internal::network::NodeId;
use wg_internal::packet::{Fragment, Packet, PacketType};

#[derive(Debug, Clone)]
pub struct Server {
    id: NodeId,
    controller_send: Sender<DroneEvent>,
    controller_recv: Receiver<DroneCommand>,
    packet_recv: Receiver<Packet>,
    senders: HashMap<NodeId, Sender<Packet>>,
    packet_forge: PacketForge,
    packets_map: HashMap<u64, Vec<Fragment>>,
    terminated: bool,
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
        }
    }

    #[must_use]
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

    fn command_dispatcher(&mut self, command: &DroneCommand) {
        match command {
            DroneCommand::Crash => {
                println!("Server {} received a crash command", self.id);
                self.terminated = true;
            }
            _ => {
                println!("Server {} unimplemented command", self.id);
            }
        }
    }

    pub fn run(&mut self) {
        loop {
            thread::sleep(Duration::from_secs(1));

            if self.terminated {
                break;
            }

            match self.controller_recv.try_recv() {
                Ok(command) => {
                    self.command_dispatcher(&command);
                }
                Err(e) => {
                    eprintln!("Error receiving command for server {}: {:?}", self.id, e);
                }
            }

            match self.packet_recv.try_recv() {
                Ok(msg) => {
                    println!("Server {} received a message: {:?}", self.id, msg);
                    self.handle_packets(msg);
                }
                Err(e) => {
                    eprintln!("Error receiving message for server {}: {:?}", self.id, e);
                }
            }

            // Message sending logic
            let text_msg =
                TextMessage::new(String::from("ciao"), String::from("30"), String::from("20"));
            if let Ok(packets) = self.packet_forge.disassemble(&text_msg, vec![30, 1, 20]) {
                for packet in packets {
                    let id = 1;
                    if let Some(sender) = self.senders.get(&id) {
                        if let Err(err) = sender.send(packet) {
                            eprintln!("Error sending packet to node {id}: {err:?}");
                        } else {
                            println!("Server {} sent packet to node {}", self.id, id);
                        }
                    } else {
                        println!("Server {} could not send packet to node {}", self.id, id);
                    }
                }
            } else {
                eprintln!("Error disassembling message: {text_msg:?}");
            }
        }
    }
}
