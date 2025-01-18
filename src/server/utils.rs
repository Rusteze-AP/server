use crate::{
    packet_send::{get_sender, sc_send_packet, send_packet},
    utils::get_packet_type,
};

use super::Server;

use packet_forge::{FileHash, FileMetadata, Metadata};
use std::collections::HashSet;
use wg_internal::{
    controller::DroneEvent,
    network::{NodeId, SourceRoutingHeader},
    packet::{Packet, PacketType},
};

impl Server {
    /*
    /// Add a new entry to `files` hashmap. If the file exists, add the new client to the peers otherwise create a new entry.
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
                entry.peers.insert(client_id);
            })
            .or_insert(FileEntry {
                file_metadata: file_metadata.clone(),
                peers: HashSet::from([client_id]),
            });
    }

    /// Remove a client from the peers of a file in the `files` hashmap.
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
            format!("Could not retrieve [Client-{client_id}] from available clients.",).as_str(),
        );
    }

    /// Remove to the given `client_id` entry in `clients` hashmap the corresponding `file_hash`
    pub(crate) fn remove_shared_file(&mut self, client_id: NodeId, file_hash: FileHash) {
        if let Some(client_info) = self.clients.get_mut(&client_id) {
            client_info.shared_files.remove(&file_hash);
        }
        self.logger.log_error(
            format!("Could not retrieve [Client-{client_id}] from available clients.",).as_str(),
        );
    }
*/
    /// Check if the received `file_hash` is correct.
    /// ### Error
    /// Log the two mismatched hash
    pub(crate) fn check_hash<M: Metadata>(
        file_hash: FileHash,
        file_metadata: &M,
    ) -> Result<(), String> {
        if file_hash != file_metadata.compact_hash_u16() {
            return Err(format!(
                "File hash mismatch! Received: [ {:?} ]\nCalculated: [ {:?} ]",
                file_hash,
                file_metadata.compact_hash_u16()
            ));
        }
        Ok(())
    }

    /// Insert a vector of packets inside the packets sent history
    /// ### Error
    /// If a `Packet` inside the vector does not contain a `Fragment` it logs an error.
    pub(crate) fn insert_packet_history(&mut self, packets: &[Packet]) {
        for p in packets {
            let PacketType::MsgFragment(fragment) = &p.pack_type else {
                self.logger.log_error(&format!(
                    "[SERVER-{}] Found {:?} while saving Fragments to packet_history! Ignoring.",
                    self.id, p.pack_type
                ));
                continue;
            };

            let key = (fragment.fragment_index, p.session_id);
            self.packets_history.insert(key, p.clone());
        }
    }

    /// Retrieve the best path from-to and log error if the path cannot be found.
    pub(crate) fn get_path(&mut self, from: NodeId, to: NodeId) -> Option<SourceRoutingHeader> {
        if let Some(srh) = self.routing_handler.best_path(from, to) {
            Some(srh)
        } else {
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

    /// Sends a `DroneEvent` containing the `packet` that has been sent.
    pub(crate) fn event_dispatcher(&self, packet: &Packet, packet_str: &str) {
        if let Err(err) = sc_send_packet(
            &self.controller_send,
            &DroneEvent::PacketSent(packet.clone()),
        ) {
            self.logger.log_error(&format!(
                "[SERVER-{}][{}] - Packet event forward: {}",
                self.id,
                packet_str.to_ascii_uppercase(),
                err
            ));
            return;
        }
        self.logger.log_debug(&format!(
            "[SERVER-{}][{}] - Packet event sent successfully",
            self.id,
            packet_str.to_ascii_uppercase()
        ));
    }

    /// Takes a vector of packets and sends them to the `next_hop`
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
            let packet_str = get_packet_type(&packet.pack_type);
            if let Err(err) = send_packet(&sender, packet) {
                return Err(format!(
                    "[SERVER-{}] Failed to send packet to [DRONE-{}].\nPacket: {}\n Error: {}",
                    self.id, next_hop, packet, err
                ));
            }
            self.event_dispatcher(packet, &packet_str);
        }
        Ok(())
    }

    /// Builds and sends an `Ack` to the `next_hop`. If it fails it tries to use the Simulation Controller
    pub(crate) fn send_ack(&mut self, packet: &Packet, fragment_index: u64) {
        let source_routing_header = packet.routing_header.get_reversed();
        if source_routing_header.hop_index != 1 {
            self.logger.log_error(format!(
                    "[SERVER-{}] - Unable to reverse source routing header. \n Hops: {} \n Hop index: {}",
                    self.id, packet.routing_header, packet.routing_header.hop_index
                ).as_str());
            return;
        }
        let next_hop = source_routing_header.hops[1];
        let ack = Packet::new_ack(source_routing_header, packet.session_id, fragment_index);

        if let Err(msg) = self.send_packets_vec(&[ack], next_hop) {
            self.logger.log_error(&msg);
            self.logger.log_debug(&format!(
                "[SERVER-{}][ACK] Trying to use SC shortcut...",
                self.id
            ));

            // Send to SC
            if let Err(msg) = sc_send_packet(
                &self.controller_send,
                &DroneEvent::ControllerShortcut(packet.clone()),
            ) {
                self.logger
                    .log_error(&format!("[SERVER-{}][ACK] - {}", self.id, msg));
                self.logger.log_error(&format!(
                    "[SERVER-{}][ACK] - Unable to forward packet to neither next hop nor SC. \n Packet: {}",
                    self.id, packet
                ));
                return;
            }

            self.logger.log_debug(
                format!(
                    "[SERVER-{}][ACK] - Successfully sent flood response through SC. Packet: {}",
                    self.id, packet
                )
                .as_str(),
            );
        }
    }

    /// This function has two purposes:
    /// - send the fragments contained within each Packet to their destination
    /// - save each packet into `packet_history`
    /// ### Error
    /// If the channel of the `next_hop` is not found returns Err(String).
    pub(crate) fn send_save_packets(
        &mut self,
        packets: &[Packet],
        next_hop: NodeId,
    ) -> Result<(), String> {
        if let Err(msg) = self.send_packets_vec(packets, next_hop) {
            return Err(msg);
        }

        self.insert_packet_history(packets);

        Ok(())
    }
}
