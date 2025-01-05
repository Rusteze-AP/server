use super::{ClientInfo, Server};

use crate::utils::check_packet_dest;
use packet_forge::*;
use std::collections::HashSet;
use wg_internal::packet::{Nack, NackType, Packet, PacketType};

impl Server {
    fn subscribe_client(&mut self, message: &SubscribeClient) {
        // Check if client is already subscribed
        if self.clients.contains_key(&message.client_id) {
            self.logger.log_warn(
                format!(
                    "[SERVER-{}] Received SubscribeClient message but [CLIENT {}] already exists!",
                    self.id, message.client_id
                )
                .as_str(),
            );
            return;
        }

        self.logger.log_debug(
            format!(
                "[SERVER-{}] Handling SubscribeClient message for [CLIENT-{}]...",
                self.id, message.client_id
            )
            .as_str(),
        );
        // Files shared by the client
        let mut shared_files = HashSet::new();

        for (file_metadata, file_hash) in &message.available_files {
            if let Err(err) = Self::check_hash(*file_hash, file_metadata) {
                self.logger
                    .log_error(format!("[SERVER-{}] {}", self.id, err).as_str());
                continue;
            }

            // Collect file_hash into shared_files
            shared_files.insert(*file_hash);

            // Insert files data into files
            self.add_to_files(message.client_id, *file_hash, file_metadata);
        }

        // Insert the client into the clients map
        self.clients.insert(
            message.client_id,
            ClientInfo {
                client_type: message.client_type.clone(),
                shared_files,
            },
        );

        self.logger.log_info(
            format!(
                "[SERVER-{}] Client {} subscribed with success!",
                self.id, message.client_id
            )
            .as_str(),
        );
    }

    fn update_file_list(&mut self, message: &UpdateFileList) {
        if !self.clients.contains_key(&message.client_id) {
            self.logger.log_warn(
                format!(
                    "[SERVER-{}] Received UpdateFileList for [CLIENT-{}] but no client was found. File list: {:?}",
                    self.id, message.client_id, message.updated_files
                )
                .as_str(),
            );
            return;
        }

        self.logger.log_debug(
            format!(
                "[SERVER-{}] Updating file list of [CLIENT-{}]...",
                self.id, message.client_id
            )
            .as_str(),
        );

        for (file_metadata, file_hash, file_status) in &message.updated_files {
            if let Err(err) = Self::check_hash(*file_hash, file_metadata) {
                self.logger
                    .log_error(format!("[SERVER-{}] {}", self.id, err).as_str());
                continue;
            }

            match file_status {
                FileStatus::New => {
                    // Update the list of files shared by `client_id`
                    self.add_shared_file(message.client_id, *file_hash);
                    // Update the file information stored in `files`
                    self.add_to_files(message.client_id, *file_hash, file_metadata);

                    self.logger.log_info(
                        format!(
                            "[SERVER-{}] Added new File [ {:?} ]",
                            self.id, file_metadata
                        )
                        .as_str(),
                    );
                }
                FileStatus::Deleted => {
                    // Update the list of files shared by `client_id`
                    self.remove_shared_file(message.client_id, *file_hash);
                    // Remove the `file_hash` entry in `files`
                    self.files.remove_entry(file_hash);

                    self.logger.log_info(
                        format!("[SERVER-{}] Removed File [ {:?} ]", self.id, file_metadata)
                            .as_str(),
                    );
                }
            };
        }
        self.logger
            .log_info(format!("[SERVER-{}] File list updated!", self.id).as_str());
    }

    fn send_file_list(&mut self, message: &RequestFileList) {
        self.logger.log_debug(
            format!("[SERVER-{}] Handling RequestFileList message...", self.id).as_str(),
        );

        let file_list = ResponseFileList::new(
            self.files
                .iter()
                .map(|(file_hash, file_entry)| (file_entry.file_metadata.clone(), *file_hash))
                .collect::<Vec<(FileMetadata, FileHash)>>(),
        );

        // Retrieve best path from server to client otherwise return
        let Some(srh) = self.get_path(self.id, message.client_id) else {
            return;
        };

        // Disassemble ResponseFileList into Packets
        let packets = match self
            .packet_forge
            .disassemble(file_list.clone(), srh.clone())
        {
            Ok(packets) => packets,
            Err(msg) => {
                self.logger.log_error(format!("[SERVER-{}] Error disassembling ResponseFileList message! (log_info to see more information)", self.id).as_str());
                self.logger.log_info(
                    format!(
                        "[SERVER-{}] ResponseFileList: {:?}\n Error: {}",
                        self.id, file_list, msg
                    )
                    .as_str(),
                );
                return;
            }
        };

        let next_hop = srh.hops[srh.hop_index];
        self.send_save_packets(&packets, next_hop);

        self.logger.log_info(
            format!(
                "[SERVER-{}] ResponseFileList procedure terminated!",
                self.id
            )
            .as_str(),
        );
    }

    fn send_peer_list(&mut self, message: &RequestPeerList) {
        self.logger.log_debug(
            format!("[SERVER-{}] Handling RequestPeerList message...", self.id).as_str(),
        );

        // Check whether the requested hash exists or not
        let file_hash = message.file_hash;
        let Some(file_entry) = self.files.get(&file_hash).cloned() else {
            self.logger.log_error(format!("[SERVER-{}] Could not find file hash [ {} ] in within files. Terminating procedure...", self.id, file_hash).as_str());
            return;
        };

        // Create the vector to send to the client
        let peers_info: Vec<PeerInfo> = file_entry
            .peers
            .iter()
            .filter_map(|peer| {
                if let Some(srh) = self.get_path(*peer, message.client_id) {
                    return Some(PeerInfo {
                        client_id: *peer,
                        path: srh.hops,
                    });
                }
                None
            })
            .collect();

        // Create response
        let file_list = ResponsePeerList::new(file_hash, peers_info);

        // Retrieve best path from server to client otherwise return
        let Some(srh) = self.get_path(self.id, message.client_id) else {
            return;
        };

        // Disassemble ResponsePeerList into Packets
        let packets = match self
            .packet_forge
            .disassemble(file_list.clone(), srh.clone())
        {
            Ok(packets) => packets,
            Err(msg) => {
                self.logger.log_error(format!("[SERVER-{}] Error disassembling ResponsePeerList message! (log_info to see more information)", self.id).as_str());
                self.logger.log_info(
                    format!(
                        "[SERVER-{}] ResponsePeerList: {:?}\n Error: {}",
                        self.id, file_list, msg
                    )
                    .as_str(),
                );
                return;
            }
        };

        let next_hop = srh.hops[srh.hop_index];
        self.send_save_packets(&packets, next_hop);

        self.logger.log_info(
            format!(
                "[SERVER-{}] ResponsePeerList procedure terminated!",
                self.id
            )
            .as_str(),
        );
    }

    fn unsubscribe_client(&mut self, message: &UnsubscribeClient) {
        // Check if client is subscribed
        if self.clients.contains_key(&message.client_id) {
            self.logger.log_warn(
                format!(
                    "[SERVER-{}] Received UnsubscribeClient message but [CLIENT {}] was not found!",
                    self.id, message.client_id
                )
                .as_str(),
            );
            return;
        }

        self.logger.log_debug(
            format!(
                "[SERVER-{}] Handling UnsubscribeClient message for [CLIENT {}]...",
                self.id, message.client_id
            )
            .as_str(),
        );

        // Remove Client from clients HashMap
        let Some(client_info) = self.clients.remove(&message.client_id) else {
            self.logger.log_error(
                format!(
                    "[SERVER-{}] No [CLIENT {}] found: could not remove it from clients!",
                    self.id, message.client_id
                )
                .as_str(),
            );
            return;
        };

        for file_hash in client_info.shared_files {
            self.logger.log_info(
                format!("[SERVER-{}] Removing file: [ {} ]", self.id, file_hash).as_str(),
            );
            self.remove_from_files(message.client_id, file_hash);
        }

        self.logger.log_info(
            format!(
                "[SERVER-{}] Client {} unsubscribed with success!",
                self.id, message.client_id
            )
            .as_str(),
        );
    }

    fn message_handler(&mut self, message: &MessageType) {
        match message {
            MessageType::SubscribeClient(msg) => {
                self.subscribe_client(msg);
            }
            MessageType::UpdateFileList(msg) => {
                self.update_file_list(msg);
            }
            MessageType::RequestFileList(msg) => {
                self.send_file_list(msg);
            }
            MessageType::RequestPeerList(msg) => {
                self.send_peer_list(msg);
            }
            MessageType::UnsubscribeClient(msg) => {
                self.unsubscribe_client(msg);
            }
            _ => {
                self.logger.log_error(
                    format!(
                        "[SERVER-{}] Unexpected message type received: {:#?}",
                        self.id, message
                    )
                    .as_str(),
                );
            }
        }
    }

    fn ack_handler(&mut self, fragment_index: u64, session_id: SessionIdT) {
        let Some(entry) = self.packets_history.remove(&(fragment_index, session_id)) else {
            self.logger.log_error(
                format!(
                    "[SERVER-{}] Failed to remove [ ({}, {}) ] key from packet history",
                    self.id, fragment_index, session_id
                )
                .as_str(),
            );
            return;
        };
        self.logger.log_info(
            format!(
                "[SERVER-{}] Packet history updated, removed: {:?}",
                self.id, entry
            )
            .as_str(),
        );
    }

    /// This function retransmit the packet for which the server received the Nack and tries to calculate a new optimal path.
    fn retransmit_packet(
        &mut self,
        packet: &mut Packet,
        fragment_index: u64,
        session_id: SessionIdT,
    ) {
        let dest = packet.routing_header.hops[packet.routing_header.hops.len()];

        // Retrieve new best path from server to client otherwise return
        let Some(srh) = self.get_path(self.id, dest) else {
            return;
        };

        let next_hop = srh.hops[srh.hop_index];
        // Assign the new SourceRoutingHeader
        packet.routing_header = srh;

        if let Err(msg) = self.send_packets_vec(&[packet.clone()], next_hop) {
            self.logger.log_error(msg.as_str());
            return;
        }

        self.logger.log_info(
            format!(
                "[SERVER-{}] Successfully re-sent packet [ ({}, {}) ]",
                self.id, fragment_index, session_id
            )
            .as_str(),
        );
    }

    fn nack_handler(&mut self, message: &Nack, session_id: SessionIdT) {
        // Retrieve the packet that generated the nack
        let Some(mut packet) = self
            .packets_history
            .get(&(message.fragment_index, session_id))
            .cloned()
        else {
            self.logger.log_error(format!("[SERVER-{}] Failed to retrieve packet with [ ({}, {}) ] key from packet history", self.id, message.fragment_index, session_id).as_str());
            return;
        };

        match message.nack_type {
            NackType::Dropped => {
                self.retransmit_packet(&mut packet, message.fragment_index, session_id);
            }
            NackType::DestinationIsDrone => {
                self.logger.log_warn(
                    format!(
                        "[SERVER-{}] Received DestinationIsDrone for {:?} ",
                        self.id, packet
                    )
                    .as_str(),
                );
            }
            NackType::ErrorInRouting(node) => {
                self.logger.log_warn(
                    format!(
                        "[SERVER-{}] Received ErrorInRouting at [NODE-{}] for {}",
                        self.id, node, packet
                    )
                    .as_str(),
                );
                // Start new flooding
                self.init_flood_request();
                // Retransmit packet
                self.retransmit_packet(&mut packet, message.fragment_index, session_id);
            }
            NackType::UnexpectedRecipient(node) => {
                self.logger.log_warn(
                    format!(
                        "[SERVER-{}] Received UnexpectedRecipient at [NODE-{}] for {}",
                        self.id, node, packet
                    )
                    .as_str(),
                );
            }
        }
    }

    pub(crate) fn packet_dispatcher(&mut self, packet: &Packet) {
        let client_id = packet.routing_header.hops[0];
        let key = (client_id, packet.session_id);

        // Check if the packet is for this server
        if !check_packet_dest(&packet.routing_header, self.id, &self.logger) {
            self.logger
                .log_info(format!("[SERVER-{}] Packet: {:?}", self.id, packet).as_str());
            return;
        }

        self.logger
            .log_info(format!("[SERVER-{}] Received: {:?}", self.id, packet).as_str());

        match &packet.pack_type {
            PacketType::MsgFragment(frag) => {
                // Save fragment
                let total_fragments = frag.total_n_fragments;
                self.packets_map.entry(key).or_default().push(frag.clone());

                // If all fragments are received, assemble the message
                let fragments = self.packets_map.get(&key).unwrap();
                if fragments.len() as u64 == total_fragments {
                    let assembled = match self.packet_forge.assemble_dynamic(fragments.clone()) {
                        Ok(message) => message,
                        Err(e) => {
                            self.logger.log_error(
                                format!(
                                    "[SERVER-{}] An error occurred when assembling fragments: {}",
                                    self.id, e
                                )
                                .as_str(),
                            );
                            return;
                        }
                    };

                    self.message_handler(&assembled);
                }
            }
            PacketType::FloodResponse(flood_res) => {
                // Update graph with flood response
                self.routing_handler.update_graph(flood_res.clone());
            }
            PacketType::FloodRequest(flood_req) => {
                // Build flood response
                self.handle_flood_request(flood_req);
            }
            PacketType::Ack(ack) => {
                // Pop the corresponding fragment from packet_history
                self.ack_handler(packet.session_id, ack.fragment_index);
            }
            PacketType::Nack(nack) => {
                // Handle different nacks
                self.nack_handler(nack, packet.session_id);
            }
        }
    }
}
