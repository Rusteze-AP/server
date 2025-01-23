use packet_forge::{ClientType, FileHash, SongMetadata, VideoMetaData};
use wg_internal::network::NodeId;

use super::{construct_payload_key, Database, FileEntry};

impl Database {
    /// Retrieves song metadata from the database by ID.
    pub(crate) fn get_song_entry(&self, id: FileHash) -> Result<FileEntry<SongMetadata>, String> {
        self.songs_tree
            .get(id.to_be_bytes())
            .map_err(|e| format!("Error accessing database: {}", e))?
            .ok_or_else(|| "Song not found".to_string())
            .and_then(|data| {
                bincode::deserialize(&data).map_err(|e| format!("Deserialization error: {}", e))
            })
    }

    // TODO `get_video_entry`
    pub(crate) fn get_video_entry(&self, id: FileHash) -> Result<FileEntry<VideoMetaData>, String> {
        todo!()
    }

    /// Retrieves song payload from the database by ID.
    pub(crate) fn get_song_payload(&self, id: FileHash) -> Result<Vec<u8>, String> {
        let key = construct_payload_key("song", id);
        self.songs_tree
            .get(key)
            .map_err(|e| format!("Error accessing database: {}", e))?
            .map(|data| data.to_vec())
            .ok_or_else(|| "Song payload not found".to_string())
    }

    // TODO `get_video_payload`
    pub(crate) fn get_video_payload(&self, id: FileHash) -> Result<Vec<u8>, String> {
        todo!()
    }

    /// Retrieves metadata for all songs in the database.
    pub(crate) fn get_all_songs_metadata(&self) -> Result<Vec<SongMetadata>, String> {
        self.songs_tree
            .iter()
            .filter_map(|entry| {
                match entry {
                    Ok((key, data)) if key.len() == 2 => {
                        bincode::deserialize(&data).ok() // Deserialize valid metadata
                    }
                    _ => None, // Skip invalid or payload entries
                }
            })
            .collect()
    }

    // TODO
    pub(crate) fn get_all_videos_metadata(&self) -> Result<Vec<VideoMetaData>, String> {
        todo!()
    }

    pub(crate) fn contains_client(&self, id: NodeId) -> bool {
        let res = self.clients_tree.contains_key(id.to_be_bytes());
        if let Err(_) = res {
            return false;
        }
        res.unwrap()
    }

    pub(crate) fn get_client_type(&self, id: NodeId) -> Result<ClientType, String> {
        self.clients_tree
            .get(id.to_be_bytes())
            .map_err(|e| format!("Error accessing database: {}", e))?
            .ok_or_else(|| "Client not found. Subscribe to the server!".to_string())
            .and_then(|data| {
                bincode::deserialize(&data).map_err(|e| format!("Deserialization error: {}", e))
            })
    }
}
