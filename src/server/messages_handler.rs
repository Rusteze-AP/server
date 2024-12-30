use super::{ClientInfo, Server};

use crate::utils::*;
use packet_forge::{MessageType, SubscribeClient};
use std::collections::HashSet;
use wg_internal::{
    config::Client,
    network::NodeId,
    packet::{Packet, PacketType},
};

impl Server {
    fn subscribe_client(&mut self, new_client_id: NodeId, client_info: SubscribeClient) {
        if self.clients.contains_key(&new_client_id) {
            self.logger.log_warn(
                format!(
                    "Received SubscribeClient message but [Client {}] already exists!",
                    new_client_id
                )
                .as_str(),
            );
        } else {
            // If the client doesn't exist, insert a new entry
            let shared_files = client_info
                .available_files
                .iter()
                .map(|(_, file_hash)| file_hash.clone())
                .collect();

            self.clients.insert(
                new_client_id,
                ClientInfo {
                    client_type: client_info.client_type,
                    shared_files,
                },
            );
        }
    }

    pub(crate) fn handle_message(&mut self, new_client_id: NodeId, message: MessageType) {
        match message {
            MessageType::SubscribeClient(client_info) => {
                self.subscribe_client(new_client_id, client_info);
            }
            MessageType::UpdateFileList(files) => {
                todo!();
            }
            MessageType::RequestFileList(files) => {
                todo!();
            }
            MessageType::RequestPeerList(file) => {
                todo!();
            }
            MessageType::UnsubscribeClient(client) => {
                todo!();
            }
            _ => {
                self.logger.log_error(
                    format!("Unexpected message type received: {:#?}", message).as_str(),
                );
            }
        }
    }

    pub(crate) fn packet_dispatcher(&mut self, packet: &Packet) {
        let new_client_id = packet.routing_header.hops[0];
        let key = (new_client_id, packet.session_id);
        match &packet.pack_type {
            PacketType::MsgFragment(frag) => {
                // Check if the packet is for this server
                if check_packet_dest(&packet.routing_header, self.id) {
                    // Save fragment
                    let total_fragments = frag.total_n_fragments;
                    self.packets_map.entry(key).or_default().push(frag.clone());

                    // If all fragments are received, assemble the message
                    let fragments = self.packets_map.get(&key).unwrap();
                    if fragments.len() == total_fragments as usize {
                        let assembled = match self.packet_forge.assemble_dynamic(fragments.clone())
                        {
                            Ok(message) => message,
                            Err(e) => panic!("Error: {e}"),
                        };

                        self.handle_message(new_client_id, assembled);
                    }
                    return;
                }

                self.logger.log_warn(
                    format!(
                        "Server {} received a fragment with destination: {:?}",
                        self.id,
                        packet.routing_header.hops.last()
                    )
                    .as_str(),
                );
            }
            _ => {
                println!("Server {} received a packet: {:?}", self.id, packet);
            }
        }
    }
}
