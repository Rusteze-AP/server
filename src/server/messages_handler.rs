use super::{ClientInfo, FileEntry, Server};

use crate::utils::*;
use logger::Logger;
use packet_forge::*;
use std::collections::{HashMap, HashSet};
use wg_internal::{
    config::Client,
    network::NodeId,
    packet::{Packet, PacketType},
};

impl Server {
    /// Add a new entry to `files` HashMap. If the file exists, add the new client to the peers otherwise create a new entry.
    fn add_to_files(
        &mut self,
        client_id: NodeId,
        file_hash: FileHash,
        file_metadata: &FileMetadata,
    ) {
        self.files
            .entry(file_hash)
            .and_modify(|entry| {
                // If the file exists, add the new client to the peers
                entry.peers.insert(client_id.clone());
            })
            .or_insert(FileEntry {
                file_metadata: file_metadata.clone(),
                peers: HashSet::from([client_id]),
            });
    }

    fn check_hash(logger: &Logger, file_hash: FileHash, file_metadata: &FileMetadata) -> bool {
        if file_hash != file_metadata.compact_hash_u16() {
            logger.log_warn(
                format!(
                    "File hash mismatch: [ {:?} ] != [ {:?} ]",
                    file_hash,
                    file_metadata.compact_hash_u16()
                )
                .as_str(),
            );
            return false;
        }
        true
    }

    fn subscribe_client(&mut self, client_id: NodeId, client_info: SubscribeClient) {
        // TODO check the correctness of the file hash and metadata
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
                if !Self::check_hash(&self.logger, file_hash, &file_metadata) {
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

    fn add_shared_file(&mut self, client_id: NodeId, file_hash: FileHash) {
        if let Some(client_info) = self.clients.get_mut(&client_id) {
            client_info.shared_files.insert(file_hash);
            return;
        }

        self.logger.log_error(
            format!(
                "Could not retrieve [Client-{}] from available clients.",
                client_id
            )
            .as_str(),
        );
    }

    fn remove_shared_file(&mut self, client_id: NodeId, file_hash: FileHash) {
        if let Some(client_info) = self.clients.get_mut(&client_id) {
            client_info.shared_files.remove(&file_hash);
        }
        self.logger.log_error(
            format!(
                "Could not retrieve [Client-{}] from available clients.",
                client_id
            )
            .as_str(),
        );
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
            if !Self::check_hash(&self.logger, file_hash, &file_metadata) {
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

    pub(crate) fn handle_message(&mut self, client_id: NodeId, message: MessageType) {
        match message {
            MessageType::SubscribeClient(client_info) => {
                self.subscribe_client(client_id, client_info);
            }
            MessageType::UpdateFileList(files) => {
                self.update_file_list(client_id, files);
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
