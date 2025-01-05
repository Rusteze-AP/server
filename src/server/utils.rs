use crate::packet_send::{get_sender, send_packet};

use super::{FileEntry, Server};

use packet_forge::*;
use std::collections::HashSet;
use wg_internal::{
    network::{NodeId, SourceRoutingHeader},
    packet::{Packet, PacketType},
};

impl Server {
    /// Add a new entry to `files` HashMap. If the file exists, add the new client to the peers otherwise create a new entry.
    pub(crate) fn add_to_files(
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

    /// Remove a client from the peers of a file in the `files` HashMap.
    /// If the file has no more peers, remove the file entry entirely.
    pub(crate) fn remove_from_files(&mut self, client_id: NodeId, file_hash: FileHash) {
        if let Some(entry) = self.files.get_mut(&file_hash) {
            // Remove the client from the peers
            entry.peers.remove(&client_id);

            // If there are no more peers, remove the file entry
            if entry.peers.is_empty() {
                self.files.remove(&file_hash);
            }
        }
    }

    /// Check if the received `file_hash` is correct.
    /// ### Error
    /// Log the two mismatched hash
    pub(crate) fn check_hash(
        file_hash: FileHash,
        file_metadata: &FileMetadata,
    ) -> Result<(), String> {
        if file_hash != file_metadata.compact_hash_u16() {
            return Err(format!(
                "File hash mismatch: [ {:?} ] != [ {:?} ]",
                file_hash,
                file_metadata.compact_hash_u16()
            ));
        }
        Ok(())
    }

    /// Add to the given `client_id` entry in `clients` hashmap a new `file_hash`
    pub(crate) fn add_shared_file(&mut self, client_id: NodeId, file_hash: FileHash) {
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

    /// Remove to the given `client_id` entry in `clients` hashmap the corresponding `file_hash`
    pub(crate) fn remove_shared_file(&mut self, client_id: NodeId, file_hash: FileHash) {
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

    /// Insert a vector of pakets inside the packets sent history
    /// ### Error
    /// If a packet inside the vector does not contain a Fragment it returns an Err(String)
    pub(crate) fn insert_packet_history(&mut self, packets: &[Packet]) -> Result<(), String> {
        for p in packets {
            let PacketType::MsgFragment(fragment) = &p.pack_type else {
                return Err(format!(
                        "[SERVER-{}] Found {:?} while saving Fragments to packet_history! Terminating routine...",
                        self.id, p.pack_type
                    ));
            };

            let key = (fragment.fragment_index, p.session_id);
            self.packets_history.insert(key, p.clone());
        }
        Ok(())
    }

    /// Retrieve the best path from-to and log error if the path cannot be found.
    pub(crate) fn get_path(&mut self, from: NodeId, to: NodeId) -> Option<SourceRoutingHeader> {
        match self.routing_handler.best_path(from, to) {
            Some(srh) => Some(srh),
            None => {
                self.logger.log_error(
                    format!(
                        "[SERVER-{}] No path found from {} to {}!",
                        self.id, self.id, to
                    )
                    .as_str(),
                );
                None
            }
        }
    }

    /// This function takes a vector of packets and sends them to the `next_hop`
    pub(crate) fn send_packets_vec(
        &mut self,
        packets: &[Packet],
        next_hop: NodeId,
    ) -> Result<(), String> {
        // Get the sender channel for the next hop and forward
        let sender = get_sender(next_hop, &self.packet_send);
        if sender.is_err() {
            return Err(format!("[SERVER-{}] {}", self.id, &sender.unwrap_err()));
        }
        let sender = sender.unwrap();

        for packet in packets {
            if let Err(err) = send_packet(&sender, &packet) {
                return Err(format!(
                    "[SERVER-{}] Failed to send packet to [DRONE-{}].\nPacket: {}\n Error: {}",
                    self.id, next_hop, packet, err
                ));
            }
        }
        Ok(())
    }

    /// This function has two purposes:
    /// - send the fragments contained within each Packet to their destination
    /// - save each packet into `packet_history`
    /// ### Error
    /// If the channel of the `next_hop` is not found it logs and returns.
    pub(crate) fn send_save_packets(&mut self, packets: &[Packet], next_hop: NodeId) {
        // Save packets into history
        if let Err(msg) = self.insert_packet_history(&packets) {
            self.logger.log_error(msg.as_str());
            return;
        }

        if let Err(msg) = self.send_packets_vec(packets, next_hop) {
            self.logger.log_error(msg.as_str());
            return;
        }
    }
}
