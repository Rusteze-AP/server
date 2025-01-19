use std::{collections::HashSet, fs};

use packet_forge::{ClientType, FileHash, Metadata, SongMetadata};
use routing_handler::Node;
use wg_internal::network::NodeId;

use super::{construct_payload_key, Database, FileEntry};

impl Database {
    /// Insert a FileEntry for SongMetadata into the songs_tree
    /// ### Checks
    /// If the entry is already present, it adds the
    pub(crate) fn insert_song_file_entry(
        &self,
        mut file_hash: FileHash,
        file_entry: &mut FileEntry<SongMetadata>,
    ) -> Result<FileHash, String> {
        if file_hash == 0 {
            file_hash = file_entry.file_metadata.compact_hash_u16();
            file_entry.file_metadata.id = file_hash;
        }

        let serialized_entry =
            bincode::serialize(&file_entry).map_err(|e| format!("Serialization error: {}", e))?;
        self.songs_tree
            .insert(file_hash.to_be_bytes(), serialized_entry)
            .map(|_| file_hash)
            .map_err(|e| format!("Error inserting song metadata: {}", e))
    }

    /// Inserts song payload into the database.
    pub(crate) fn insert_song_payload(&self, id: FileHash, payload: Vec<u8>) -> Result<(), String> {
        let key = construct_payload_key("song", id);
        match self.songs_tree.insert(key, payload) {
            Ok(_) => Ok(()),
            Err(e) => Err(format!("Error inserting song payload: {}", e)),
        }
    }

    /// Insert a vector of `SongMetadata` inside `audio_tree` if any path is provided it tries to insert also the payloads
    pub(crate) fn insert_songs_from_vec(
        &self,
        local_path: &str,
        songs: &Vec<SongMetadata>,
    ) -> Result<(), String> {
        for song in songs {
            let mut file_entry = FileEntry {
                file_metadata: song.clone(),
                peers: HashSet::new(),
            };

            let song_id = self.insert_song_file_entry(song.id, &mut file_entry)?;

            let song_title_parsed = song.title.replace(' ', "").to_lowercase();
            let mp3_file_path = format!("{}/songs/{}.mp3", local_path, song_title_parsed);

            let mp3_content = fs::read(&mp3_file_path)
                .map_err(|e| format!("Error reading MP3 file {}: {}", mp3_file_path, e))?;

            self.insert_song_payload(song_id, mp3_content)?;
        }
        Ok(())
    }

    /// Add the `client_id` to the entry `song_metadata` in the database. If not present inserts a new entry.
    pub(crate) fn insert_song_peer(
        &self,
        song_metadata: &SongMetadata,
        client_id: NodeId,
    ) -> Result<(), String> {
        // Attempt to retrieve the existing song entry
        let mut file_entry = match self.get_song_entry(song_metadata.id) {
            Ok(mut entry) => {
                // Add the client to the peers if the entry exists
                entry.peers.insert(client_id);
                entry
            }
            Err(_) => {
                // If the entry does not exist, create a new FileEntry
                let new_entry = FileEntry {
                    file_metadata: song_metadata.clone(),
                    peers: HashSet::from([client_id]),
                };
                new_entry
            }
        };

        // Update or insert the FileEntry in the songs_tree
        self.insert_song_file_entry(song_metadata.id, &mut file_entry)?;

        Ok(())
    }

    pub(crate) fn remove_song(&self, id: FileHash) -> Result<(), String> {
        if let Err(msg) = self.songs_tree.remove(id.to_be_bytes()) {
            return Err(format!("An error occurred while removing a song: {}", msg));
        }
        Ok(())
    }

    pub(crate) fn remove_peer_from_songs(&self, peer_id: NodeId) -> Result<(), String> {
        let mut errors: Vec<String> = Vec::new();

        // Process songs_tree
        for entry in self.songs_tree.iter() {
            match entry {
                Ok((_, value)) => {
                    let mut file_entry: FileEntry<SongMetadata> = match bincode::deserialize(&value)
                    {
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
                            .insert_song_file_entry(file_entry.file_metadata.id, &mut file_entry)
                        {
                            errors.push(format!("Error updating song entry: {}", e));
                        }
                    }
                }
                Err(e) => {
                    errors.push(format!("Error iterating songs_tree: {}", e));
                }
            }
        }
        // Log or handle collected errors
        if !errors.is_empty() {
            for error in &errors {
                // TODO use logger
                eprintln!("{}", error); // Log errors
            }
            return Err(format!("Completed with {} errors", errors.len()));
        }

        Ok(())
    }

    // TODO: missing VideoMetadata struct
    /// Insert a FileEntry for VideoMetadata into the video_tree
    // pub(crate) fn insert_video_file_entry(&self, mut file_hash: FileHash, file_entry: &mut FileEntry<VideoMetadata>) -> Result<(), String> {
    // if file_hash == 0 {
    //     file_hash = file_entry.file_metadata.compact_hash_u16();
    //     file_entry.file_metadata.id = file_hash;
    // }

    // let serialized_entry =
    //     bincode::serialize(&file_entry).map_err(|e| format!("Serialization error: {}", e))?;
    // self.video_tree
    //     .insert(file_hash.to_be_bytes(), serialized_entry)
    //     .map(|_| file_hash)
    //     .map_err(|e| format!("Error inserting song metadata: {}", e))
    // }

    // TODO `insert_video_payload`

    // pub(crate) fn insert_videos_from_vec(
    //     &self,
    //     local_path: &str,
    //     videos: &Vec<VideoMetadata>,
    // ) -> Result<(), String> {
    //     for video in videos {
    //         let video_title_parsed = video.title.replace(' ', "").to_lowercase();
    //         let video_file_path = format!("{}/videos/{}.mp4", local_path, video_title_parsed);

    //         let video_content = fs::read(&video_file_path)
    //             .map_err(|e| format!("Error reading video file {}: {}", video_file_path, e))?;

    //         let video_id = self.insert_video_metadata(video)?;
    //         self.insert_video_payload(video_id, video_content)?;
    //     }
    //     Ok(())
    // }

    pub(crate) fn insert_video_peer(
        &self,
        song_metadata: &SongMetadata,
        client_id: NodeId,
    ) -> Result<(), String> {
        todo!()
    }

    pub(crate) fn remove_video(&self, peer_id: FileHash) -> Result<(), String> {
        todo!()
    }

    // pub(crate) fn remove_peer_from_videos(&self, peer_id: NodeId) -> Result<(), String> {
    //     let mut errors: Vec<String> = Vec::new();

    //     // Process songs_tree
    //     for entry in self.video_tree.iter() {
    //         match entry {
    //             Ok((_, value)) => {
    //                 let mut file_entry: FileEntry<SongMetadata> = match bincode::deserialize(&value) {
    //                     Ok(fe) => fe,
    //                     Err(e) => {
    //                         errors.push(format!("Deserialization error: {}", e));
    //                         continue; // Skip this entry
    //                     }
    //                 };

    //                 // Remove the peer if it exists
    //                 if file_entry.peers.remove(&peer_id) {
    //                     // Re-insertion with edited peer list
    //                     if let Err(e) = self.insert_video_file_entry(
    //                         file_entry.file_metadata.id,
    //                         &mut file_entry,
    //                     ) {
    //                         errors.push(format!("Error updating song entry: {}", e));
    //                     }
    //                 }
    //             }
    //             Err(e) => {
    //                 errors.push(format!("Error iterating songs_tree: {}", e));
    //             }
    //         }
    //     }
    //     // Log or handle collected errors
    //     if !errors.is_empty() {
    //         for error in &errors {
    //             // TODO use logger
    //             eprintln!("{}", error); // Log errors
    //         }
    //         return Err(format!("Completed with {} errors", errors.len()));
    //     }

    //     Ok(())
    // }

    /// Insert the client into the tree. To use after the use of `contains_client`, this will replace any previous entry.
    pub(crate) fn insert_client(&self, id: NodeId, client_type: ClientType) -> Result<(), String> {
        let serialized_type =
            bincode::serialize(&client_type).map_err(|e| format!("Serialization error: {}", e))?;
        let _ = self
            .clients_tree
            .insert(id.to_be_bytes(), serialized_type)
            .map_err(|e| format!("Error inserting song metadata: {}", e));
        Ok(())
    }

    pub(crate) fn remove_client(&self, id: NodeId) -> Result<Option<ClientType>, String> {
        match self.clients_tree.remove(id.to_be_bytes()) {
            Ok(Some(removed_value)) => {
                // Deserialize the removed value into ClientType
                bincode::deserialize(&removed_value)
                    .map(Some) // Wrap the ClientType in Option
                    .map_err(|e| format!("Deserialization error: {}", e)) // Handle deserialization errors
            }
            Ok(None) => Ok(None), // No client found, return None
            Err(msg) => Err(format!(
                "An error occurred while removing the client: {}",
                msg
            )),
        }
    }
}
