use packet_forge::{ClientType, FileHash, SongMetaData, VideoMetaData};
use wg_internal::network::NodeId;

use super::{construct_payload_key, Database, FileEntry};

impl Database {
    /// Retrieves song metadata from the database by ID.
    pub(crate) fn get_song_entry(&self, id: FileHash) -> Result<FileEntry<SongMetaData>, String> {
        self.songs_tree
            .get(id.to_be_bytes())
            .map_err(|e| format!("Error accessing database: {e}"))?
            .ok_or_else(|| "Song not found".to_string())
            .and_then(|data| {
                bincode::deserialize(&data).map_err(|e| format!("Deserialization error: {e}"))
            })
    }

    /// Retrieves video metadata from the database by ID.
    pub(crate) fn get_video_entry(&self, id: FileHash) -> Result<FileEntry<VideoMetaData>, String> {
        self.video_tree
            .get(id.to_be_bytes())
            .map_err(|e| format!("Error accessing database: {e}"))?
            .ok_or_else(|| "Video not found".to_string())
            .and_then(|data| {
                bincode::deserialize(&data).map_err(|e| format!("Deserialization error: {e}"))
            })
    }

    /// Retrieves song payload from the database by ID.
    pub(crate) fn get_song_payload(&self, prefix: &str, id: FileHash) -> Result<Vec<u8>, String> {
        let key = construct_payload_key(prefix, id);
        self.songs_tree
            .get(key)
            .map_err(|e| format!("Error accessing database: {e}"))?
            .map(|data| data.to_vec())
            .ok_or_else(|| "Song payload not found".to_string())
    }

    /// Retrieves video payload from the database by ID.
    pub(crate) fn get_video_payload(&self, id: FileHash) -> Result<Vec<u8>, String> {
        let key = construct_payload_key("pl", id);
        self.video_tree
            .get(key)
            .map_err(|e| format!("Error accessing database: {e}"))?
            .map(|data| data.to_vec())
            .ok_or_else(|| "Video payload not found".to_string())
    }

    /// Retrieves metadata for all songs in the database, excluding those shared by the specified node.
    pub(crate) fn get_songs_filtered(&self, node_id: NodeId) -> Vec<SongMetaData> {
        self.songs_tree
            .iter()
            .filter_map(|entry| {
                if let Ok((key, data)) = entry {
                    if let Ok(key_str) = std::str::from_utf8(&key) {
                        if key_str.starts_with("ts") {
                            return None; // Skip entries where key starts with "ts"
                        }
                    }

                    // Attempt to deserialize the data
                    if let Ok(file_entry) = bincode::deserialize::<FileEntry<SongMetaData>>(&data) {
                        // Exclude entries where node_id is present in peers
                        if !file_entry.peers.contains(&node_id) {
                            return Some(file_entry.file_metadata);
                        }
                    }
                }
                None
            })
            .collect()
    }

    /// Retrieves metadata for all videos in the database, excluding those shared by the specified node.
    pub(crate) fn get_videos_filtered(&self, node_id: NodeId) -> Vec<VideoMetaData> {
        self.video_tree
            .iter()
            .filter_map(|entry| {
                if let Ok((key, data)) = entry {
                    if let Ok(key_str) = std::str::from_utf8(&key) {
                        if key_str.starts_with("pl") {
                            return None; // Skip entries where key starts with "pl"
                        }
                    }

                    // Attempt to deserialize the data
                    if let Ok(file_entry) = bincode::deserialize::<FileEntry<VideoMetaData>>(&data)
                    {
                        // Exclude entries where node_id is present in peers
                        if !file_entry.peers.contains(&node_id) {
                            return Some(file_entry.file_metadata);
                        }
                    }
                }

                None
            })
            .collect()
    }

    pub(crate) fn contains_client(&self, id: NodeId) -> bool {
        let res = self.clients_tree.contains_key(id.to_be_bytes());
        if res.is_err() {
            return false;
        }
        res.unwrap()
    }

    pub(crate) fn get_client_type(&self, id: NodeId) -> Result<ClientType, String> {
        self.clients_tree
            .get(id.to_be_bytes())
            .map_err(|e| format!("Error accessing database: {e}"))?
            .ok_or_else(|| "Client not found. Subscribe to the server!".to_string())
            .and_then(|data| {
                bincode::deserialize(&data).map_err(|e| format!("Deserialization error: {e}"))
            })
    }
}
