use super::{FileEntry, Server};

use packet_forge::*;
use std::collections::HashSet;
use wg_internal::network::NodeId;

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
}
