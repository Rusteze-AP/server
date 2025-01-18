use packet_forge::{FileHash, SongMetadata};
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

    pub(crate) fn contains_client(&self, id: NodeId) -> bool {
        let res = self.clients_tree.contains_key(id.to_be_bytes());
        if let Err(_) = res {
            return false;
        }
        res.unwrap()
    }
}