use std::fs::File;
use std::io::{self, BufReader, Read};
use std::path::PathBuf;
/// Iterator for extracting strings from a file
pub struct FileStringsIterator {
    reader: BufReader<File>, // File reader with buffering
    buffer: [u8; 8192],      // 8KB buffer for reading chunks
    string_buffer: String,   // Buffer to accumulate characters for the current string
    current_block: Vec<u8>,  // Current chunk of data being processed
    block_index: usize,      // Index to track the current position in the chunk
}

impl FileStringsIterator {
    /// Creates a new iterator for the given file path
    pub fn new(file_path: PathBuf) -> io::Result<Self> {
        let file = File::open(file_path)?; // Open the file
        let reader = BufReader::new(file);
        Ok(Self {
            reader,
            buffer: [0; 8192], // Initialize the buffer
            string_buffer: String::new(),
            current_block: Vec::new(),
            block_index: 0,
        })
    }

    /// Reads the next chunk of data from the file into the buffer
    fn read_next_block(&mut self) -> io::Result<()> {
        // Read a chunk of data into the buffer
        let n = self.reader.read(&mut self.buffer)?;
        self.current_block = self.buffer[..n].to_vec(); // Copy the data to the current block
        self.block_index = 0; // Reset the index for processing the new chunk
        Ok(())
    }
}

impl Iterator for FileStringsIterator {
    type Item = String;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            // If the current chunk is fully processed, read the next chunk
            if self.block_index >= self.current_block.len() {
                if let Err(err) = self.read_next_block() {
                    eprintln!("Failed to read file: {}", err);
                    return None;
                }
                // If the end of the file is reached, return the remaining string in the buffer
                if self.current_block.is_empty() {
                    return if self.string_buffer.len() >= 4 {
                        Some(self.string_buffer.drain(..).collect())
                    } else {
                        None
                    };
                }
            }

            // Process the current byte
            let byte = self.current_block[self.block_index];
            self.block_index += 1;

            // If the byte is a printable ASCII character (32-126), add it to the string buffer
            if byte >= 32 && byte <= 126 {
                self.string_buffer.push(byte as char);
            } else {
                // If the string buffer has at least 4 characters, return it as a string
                if self.string_buffer.len() >= 4 {
                    return Some(self.string_buffer.drain(..).collect());
                }
                self.string_buffer.clear(); // Clear the buffer for the next string
            }
        }
    }
}