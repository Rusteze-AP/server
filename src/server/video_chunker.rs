use bytes::{Bytes, BytesMut};
use std::io::{self, Cursor, Read};

pub struct VideoChunker {
    data: Cursor<Vec<u8>>,
    chunk_size: usize,
    position: u64,
    data_size: u64,
}

impl VideoChunker {
    pub fn new(video_data: Vec<u8>, chunk_size: usize) -> Self {
        let data_size = video_data.len() as u64;
        VideoChunker {
            data: Cursor::new(video_data),
            chunk_size,
            position: 0,
            data_size,
        }
    }

    pub fn next_chunk(&mut self) -> io::Result<Option<Bytes>> {
        if self.position >= self.data_size {
            return Ok(None);
        }

        let mut buffer = BytesMut::with_capacity(self.chunk_size);
        buffer.resize(self.chunk_size, 0);

        self.data.set_position(self.position);
        let bytes_read = self.data.read(&mut buffer)?;

        if bytes_read == 0 {
            return Ok(None);
        }

        buffer.truncate(bytes_read);
        self.position += bytes_read as u64;

        Ok(Some(buffer.freeze()))
    }

    pub fn reset(&mut self) {
        self.position = 0;
        self.data.set_position(0);
    }
}

pub struct ChunkIterator {
    chunker: VideoChunker,
}

impl Iterator for ChunkIterator {
    type Item = Bytes;

    fn next(&mut self) -> Option<Self::Item> {
        if let Ok(Some(chunk)) = self.chunker.next_chunk() {
            Some(chunk)
        } else {
            self.chunker.reset();
            None
        }
    }
}

impl ExactSizeIterator for ChunkIterator {
    fn len(&self) -> usize {
        (self.chunker.data_size as usize + self.chunker.chunk_size - 1) / self.chunker.chunk_size
    }
}

pub fn get_video_chunks(video_data: Vec<u8>) -> ChunkIterator {
    // Create the chunker with a 256KB chunk size
    let chunker = VideoChunker::new(video_data, 512 * 512);
    ChunkIterator { chunker }
}
