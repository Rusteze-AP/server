use crate::ClientInfo;

use super::Server;

use packet_forge::*;
use std::collections::HashSet;

impl Server {
    pub(crate) fn subscribe_client(&mut self, message: &SubscribeClient) {
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

    pub(crate) fn update_file_list(&mut self, message: &UpdateFileList) {
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

    pub(crate) fn send_file_list(&mut self, message: &RequestFileList) {
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

    pub(crate) fn send_peer_list(&mut self, message: &RequestPeerList) {
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

    pub(crate) fn unsubscribe_client(&mut self, message: &UnsubscribeClient) {
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
}
