use std::{collections::HashSet, fs};

use packet_forge::{FileHash, Metadata, VideoMetaData};
use wg_internal::network::NodeId;

use super::{construct_payload_key, Database, FileEntry};

impl Database {
    /// Insert a FileEntry for VideoMetaData into the video_tree
    fn insert_video_file_entry(
        &self,
        mut file_hash: FileHash,
        file_entry: &mut FileEntry<VideoMetaData>,
    ) -> Result<FileHash, String> {
        if file_hash == 0 {
            file_hash = file_entry.file_metadata.compact_hash_u16();
            file_entry.file_metadata.id = file_hash;
        }

        let serialized_entry =
            bincode::serialize(&file_entry).map_err(|e| format!("Serialization error: {}", e))?;
        self.video_tree
            .insert(file_hash.to_be_bytes(), serialized_entry)
            .map(|_| file_hash)
            .map_err(|e| format!("Error inserting song metadata: {}", e))
    }

    /// Inserts video payload into the database.
    fn insert_video_payload(&self, id: FileHash, payload: Vec<u8>) -> Result<(), String> {
        let key = construct_payload_key("video", id);
        match self.video_tree.insert(key, payload) {
            Ok(_) => Ok(()),
            Err(e) => Err(format!("Error inserting song payload: {}", e)),
        }
    }

    /// Insert a vector of `VideoMetaData` inside `video_tree`.
    pub(crate) fn insert_videos_from_vec(
        &self,
        local_path: &str,
        videos: &Vec<VideoMetaData>,
    ) -> Result<(), String> {
        for video in videos {
            let mut file_entry = FileEntry {
                file_metadata: video.clone(),
                peers: HashSet::from([self.server_id]),
            };

            let video_id = self.insert_video_file_entry(video.id, &mut file_entry)?;

            let video_title_parsed = video.title.replace(' ', "").to_lowercase();
            let video_file_path = format!("{}/videos/{}.mp4", local_path, video_title_parsed);

            let video_content = fs::read(&video_file_path)
                .map_err(|e| format!("Error reading video file {}: {}", video_file_path, e))?;

            self.insert_video_payload(video_id, video_content)?;
        }
        Ok(())
    }

    /// Add the `peer_id` to the entry `video_metadata` in the database. If not present inserts a new entry.
    pub(crate) fn insert_video_peer(
        &self,
        video_metadata: &VideoMetaData,
        peer_id: NodeId,
    ) -> Result<(), String> {
        // Attempt to retrieve the existing song entry
        let mut file_entry = match self.get_video_entry(video_metadata.id) {
            Ok(mut entry) => {
                // Add the client to the peers if the entry exists
                entry.peers.insert(peer_id);
                entry
            }
            Err(_) => {
                // If the entry does not exist, create a new FileEntry
                let new_entry = FileEntry {
                    file_metadata: video_metadata.clone(),
                    peers: HashSet::from([peer_id]),
                };
                new_entry
            }
        };

        // Update or insert the FileEntry in the video_tree
        self.insert_video_file_entry(video_metadata.id, &mut file_entry)?;

        Ok(())
    }

    pub(crate) fn remove_video(&self, peer_id: FileHash) -> Result<(), String> {
        if let Err(msg) = self.video_tree.remove(peer_id.to_be_bytes()) {
            return Err(format!("An error occurred while removing a video: {}", msg));
        }
        Ok(())
    }

    pub(crate) fn remove_peer_from_videos(&self, peer_id: NodeId) -> Result<(), String> {
        let mut errors: Vec<String> = Vec::new();

        // Process video_tree
        for entry in self.video_tree.iter() {
            match entry {
                Ok((_, value)) => {
                    let mut file_entry: FileEntry<VideoMetaData> =
                        match bincode::deserialize(&value) {
                            Ok(fe) => fe,
                            Err(e) => {
                                errors.push(format!("Deserialization error: {}", e));
                                continue; // Skip this entry
                            }
                        };

                    // Remove the peer if it exists
                    if file_entry.peers.remove(&peer_id) {
                        // Re-insertion with edited peer list
                        if let Err(e) = self
                            .insert_video_file_entry(file_entry.file_metadata.id, &mut file_entry)
                        {
                            errors.push(format!("Error updating video entry: {}", e));
                        }
                    }
                }
                Err(e) => {
                    errors.push(format!("Error iterating video_tree: {}", e));
                }
            }
        }
        // Log or handle collected errors
        if !errors.is_empty() {
            for error in &errors {
                // TODO use logger
                eprintln!("{}", error); // Log errors
            }
            return Err(format!(
                "Remove peer from video completed with {} errors",
                errors.len()
            ));
        }

        Ok(())
    }
}
