use crossbeam::channel::{Receiver, Sender, TryRecvError};
use std::{collections::HashMap, thread, time::Duration};
use wg_internal::controller::{DroneCommand, DroneEvent};
use wg_internal::network::NodeId;
use wg_internal::packet::Packet;

pub struct Server {
    id: NodeId,
    command_send: Sender<DroneEvent>,
    command_recv: Receiver<DroneCommand>,
    receiver: Receiver<Packet>,
    senders: HashMap<NodeId, Sender<Packet>>,
}

impl Server {
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
        }
    }

    pub fn get_id(&self) -> NodeId {
        self.id
    }

    pub fn run(&self) {
        loop {
            thread::sleep(Duration::from_secs(1));

            // Check if there's a message from the drone
            match self.receiver.try_recv() {
                Ok(packet) => {
                    println!("Server {} received a message: {:?}", self.id, packet);
                }
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
