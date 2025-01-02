use super::{ClientInfo, Server};

use crate::utils::*;
use packet_forge::*;
use std::collections::HashSet;
use wg_internal::{
    network::NodeId,
    packet::{Packet, PacketType},
};

impl Server {
    fn subscribe_client(&mut self, client_id: NodeId, client_info: SubscribeClient) {
        // Check if client is already subscribed
        if self.clients.contains_key(&client_id) {
            self.logger.log_warn(
                format!(
                    "Received SubscribeClient message but [Client {}] already exists!",
                    client_id
                )
                .as_str(),
            );
        } else {
            // Files shared by the client
            let mut shared_files = HashSet::new();

            for (file_metadata, file_hash) in client_info.available_files {
                if let Err(err) = Self::check_hash(file_hash, &file_metadata) {
                    self.logger.log_error(err.as_str());
                    continue;
                }

                // Collect file_hash into shared_files
                shared_files.insert(file_hash);

                // Insert files data into files
                self.add_to_files(client_id, file_hash, &file_metadata);
            }

            // Insert the client into the clients map
            self.clients.insert(
                client_id,
                ClientInfo {
                    client_type: client_info.client_type,
                    shared_files,
                },
            );
        }
    }

    fn update_file_list(&mut self, client_id: NodeId, files: UpdateFileList) {
        self.logger
            .log_debug(format!("Updating file list of [Client-{}]...", client_id).as_str());

        if !self.clients.contains_key(&client_id) {
            self.logger.log_warn(
                format!(
                    "Received UpdateFileList for [Client-{}] but no client was found. File list: {:?}",
                    client_id, files.updated_files
                )
                .as_str(),
            );
            return;
        }

        for (file_metadata, file_hash, file_status) in files.updated_files {
            if let Err(err) = Self::check_hash(file_hash, &file_metadata) {
                self.logger.log_error(err.as_str());
                continue;
            }

            match file_status {
                FileStatus::New => {
                    // Update the list of files shared by `client_id`
                    self.add_shared_file(client_id, file_hash);
                    // Update the file information stored in `files`
                    self.add_to_files(client_id, file_hash, &file_metadata);

                    self.logger
                        .log_info(format!("Added new File [ {:?} ]", file_metadata).as_str());
                }
                FileStatus::Deleted => {
                    // Update the list of files shared by `client_id`
                    self.remove_shared_file(client_id, file_hash);
                    // Remove the `file_hash` entry in `files`
                    self.files.remove_entry(&file_hash);

                    self.logger
                        .log_info(format!("Removed File [ {:?} ]", file_metadata).as_str());
                }
            };
        }
        self.logger.log_debug("File list updated!");
    }

    fn send_file_list(&self) {
        todo!()
    }

    pub(crate) fn handle_message(&mut self, client_id: NodeId, message: MessageType) {
        match message {
            MessageType::SubscribeClient(client_info) => {
                self.subscribe_client(client_id, client_info);
            }
            MessageType::UpdateFileList(files) => {
                self.update_file_list(client_id, files);
            }
            MessageType::RequestFileList(files) => {
                self.send_file_list();
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
        let client_id = packet.routing_header.hops[0];
        let key = (client_id, packet.session_id);
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

                        self.handle_message(client_id, assembled);
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
