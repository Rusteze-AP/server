use std::io::{self, Read, Seek, SeekFrom};
use std::{fs::File, path::Path};

use bytes::{Bytes, BytesMut};

pub struct VideoChunker {
    file: File,
    chunk_size: usize,
    position: u64,
    file_size: u64,
}

impl VideoChunker {
    pub fn new(path: impl AsRef<Path>, chunk_size: usize) -> io::Result<Self> {
        let file = File::open(path)?;
        let file_size = file.metadata()?.len();

        Ok(VideoChunker {
            file,
            chunk_size,
            position: 0,
            file_size,
        })
    }

    pub fn next_chunk(&mut self) -> io::Result<Option<Bytes>> {
        if self.position >= self.file_size {
            return Ok(None);
        }

        let mut buffer = BytesMut::with_capacity(self.chunk_size);
        buffer.resize(self.chunk_size, 0);

        self.file.seek(SeekFrom::Start(self.position))?;
        let bytes_read = self.file.read(&mut buffer)?;

        if bytes_read == 0 {
            return Ok(None);
        }

        buffer.truncate(bytes_read);
        self.position += bytes_read as u64;

        Ok(Some(buffer.freeze()))
    }

    pub fn reset(&mut self) -> io::Result<()> {
        self.position = 0;
        self.file.seek(SeekFrom::Start(0))?;
        Ok(())
    }
}

struct ChunkIterator {
    chunker: VideoChunker,
}

impl Iterator for ChunkIterator {
    type Item = Bytes;

    fn next(&mut self) -> Option<Self::Item> {
        if let Ok(Some(chunk)) = self.chunker.next_chunk() {
            Some(chunk)
        } else {
            let _ = self.chunker.reset();
            None
        }
    }
}

pub(crate) fn get_video_chunks(file_hash: u16) -> impl Iterator<Item = Bytes> {
    // TODO get path from database using `file_hash`

    let path = "../client/static/videos/dancing_pirate.mp4";
    // Create the chunker with a 1MB chunk size
    let chunker = VideoChunker::new(path, 1024 * 1024).expect("Failed to create video chunker");

    ChunkIterator { chunker }
}
