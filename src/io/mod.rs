//! I/O utilities module
//!
//! This module provides efficient file access utilities including memory-mapped
//! file I/O for optimal performance.

use memmap2::{Mmap, MmapOptions};
use std::fs::File;
use std::path::{Path, PathBuf};
use thiserror::Error;

/// Type alias for buffer offset positions
type BufferOffset = usize;

/// Type alias for buffer lengths
type BufferLength = usize;

/// Type alias for file sizes in bytes
type FileSize = u64;

/// Internal trait for safe buffer operations
trait SafeBufferAccess {
    /// Validates that an access operation is within bounds
    fn validate_access(&self, offset: BufferOffset, length: BufferLength) -> Result<(), IoError>;

    /// Gets a safe slice from the buffer
    fn get_safe_slice(&self, offset: BufferOffset, length: BufferLength) -> Result<&[u8], IoError>;
}

impl SafeBufferAccess for [u8] {
    fn validate_access(&self, offset: BufferOffset, length: BufferLength) -> Result<(), IoError> {
        validate_buffer_access(self.len(), offset, length)
    }

    fn get_safe_slice(&self, offset: BufferOffset, length: BufferLength) -> Result<&[u8], IoError> {
        self.validate_access(offset, length)?;
        let end_offset = offset + length; // Safe after validation
        Ok(&self[offset..end_offset])
    }
}

/// Errors that can occur during file I/O operations
#[derive(Debug, Error)]
pub enum IoError {
    /// File could not be opened for reading
    #[error("Failed to open file '{path}': {source}")]
    FileOpenError {
        /// Path to the file that could not be opened
        path: PathBuf,
        /// Underlying I/O error
        #[source]
        source: std::io::Error,
    },

    /// File could not be memory-mapped
    #[error("Failed to memory-map file '{path}': {source}")]
    MmapError {
        /// Path to the file that could not be mapped
        path: PathBuf,
        /// Underlying I/O error
        #[source]
        source: std::io::Error,
    },

    /// File is empty and cannot be processed
    #[error("File '{path}' is empty")]
    EmptyFile {
        /// Path to the empty file
        path: PathBuf,
    },

    /// File is too large to be processed safely
    #[error("File '{path}' is too large ({size} bytes, maximum {max_size} bytes)")]
    FileTooLarge {
        /// Path to the file that is too large
        path: PathBuf,
        /// Actual file size in bytes
        size: FileSize,
        /// Maximum allowed file size in bytes
        max_size: FileSize,
    },

    /// File metadata could not be read
    #[error("Failed to read metadata for file '{path}': {source}")]
    MetadataError {
        /// Path to the file whose metadata could not be read
        path: PathBuf,
        /// Underlying I/O error
        #[source]
        source: std::io::Error,
    },

    /// Buffer access out of bounds
    #[error(
        "Buffer access out of bounds: offset {offset} + length {length} > buffer size {buffer_size}"
    )]
    BufferOverrun {
        /// Requested offset
        offset: BufferOffset,
        /// Requested length
        length: BufferLength,
        /// Actual buffer size
        buffer_size: BufferLength,
    },

    /// Invalid offset or length parameter
    #[error("Invalid buffer access parameters: offset {offset}, length {length}")]
    InvalidAccess {
        /// Requested offset
        offset: BufferOffset,
        /// Requested length
        length: BufferLength,
    },
}

/// A memory-mapped file buffer for efficient file access
///
/// This struct provides safe access to file contents through memory mapping,
/// which avoids loading the entire file into memory while providing fast
/// random access to file data.
///
/// # Examples
///
/// ```no_run
/// use libmagic_rs::io::FileBuffer;
/// use std::path::Path;
///
/// let buffer = FileBuffer::new(Path::new("example.bin"))?;
/// let data = buffer.as_slice();
/// println!("File size: {} bytes", data.len());
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug)]
pub struct FileBuffer {
    /// Memory-mapped file data
    mmap: Mmap,
    /// Path to the file for error reporting
    path: PathBuf,
}

impl FileBuffer {
    /// Maximum file size that can be processed (1 GB)
    const MAX_FILE_SIZE: FileSize = 1024 * 1024 * 1024;

    // TODO: Consider implementing adaptive I/O strategy for small files
    // Files smaller than 4KB might benefit from regular read() instead of mmap
    // This would require benchmarking to determine the optimal threshold

    /// Creates a new memory-mapped file buffer
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the file to be mapped
    ///
    /// # Returns
    ///
    /// Returns a `FileBuffer` on success, or an `IoError` if the file cannot
    /// be opened or mapped.
    ///
    /// # Errors
    ///
    /// This function will return an error if:
    /// - The file does not exist or cannot be opened
    /// - The file cannot be memory-mapped
    /// - The file is empty
    /// - The file is larger than the maximum allowed size
    /// - File metadata cannot be read
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use libmagic_rs::io::FileBuffer;
    /// use std::path::Path;
    ///
    /// let buffer = FileBuffer::new(Path::new("example.bin"))?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn new(path: &Path) -> Result<Self, IoError> {
        let path_buf = path.to_path_buf();

        let file = Self::open_file(path, &path_buf)?;
        Self::validate_file_metadata(&file, &path_buf)?;
        let mmap = Self::create_memory_mapping(&file, &path_buf)?;

        Ok(Self {
            mmap,
            path: path_buf,
        })
    }

    /// Opens a file for reading with proper error handling
    fn open_file(path: &Path, path_buf: &Path) -> Result<File, IoError> {
        File::open(path).map_err(|source| IoError::FileOpenError {
            path: path_buf.to_path_buf(),
            source,
        })
    }

    /// Validates file metadata and ensures file is suitable for memory mapping
    fn validate_file_metadata(file: &File, path_buf: &Path) -> Result<(), IoError> {
        let metadata = file.metadata().map_err(|source| IoError::MetadataError {
            path: path_buf.to_path_buf(),
            source,
        })?;

        let file_size = metadata.len();

        // Check if file is empty
        if file_size == 0 {
            return Err(IoError::EmptyFile {
                path: path_buf.to_path_buf(),
            });
        }

        // Check if file is too large
        if file_size > Self::MAX_FILE_SIZE {
            return Err(IoError::FileTooLarge {
                path: path_buf.to_path_buf(),
                size: file_size,
                max_size: Self::MAX_FILE_SIZE,
            });
        }

        Ok(())
    }

    /// Creates memory mapping for the file
    fn create_memory_mapping(file: &File, path_buf: &Path) -> Result<Mmap, IoError> {
        // SAFETY: We use safe memory mapping through memmap2, which handles
        // the unsafe operations internally with proper error checking.
        // The memmap2 crate is a vetted dependency that provides safe abstractions
        // over unsafe memory mapping operations.
        #[allow(unsafe_code)]
        unsafe {
            MmapOptions::new()
                .map(file)
                .map_err(|source| IoError::MmapError {
                    path: path_buf.to_path_buf(),
                    source,
                })
        }
    }

    /// Returns the file contents as a byte slice
    ///
    /// This provides safe access to the memory-mapped file data without
    /// copying the contents.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use libmagic_rs::io::FileBuffer;
    /// use std::path::Path;
    ///
    /// let buffer = FileBuffer::new(Path::new("example.bin"))?;
    /// let data = buffer.as_slice();
    /// println!("First byte: 0x{:02x}", data[0]);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[must_use]
    pub fn as_slice(&self) -> &[u8] {
        &self.mmap
    }

    /// Returns the path of the file
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use libmagic_rs::io::FileBuffer;
    /// use std::path::Path;
    ///
    /// let buffer = FileBuffer::new(Path::new("example.bin"))?;
    /// println!("File path: {}", buffer.path().display());
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Returns the size of the file in bytes
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use libmagic_rs::io::FileBuffer;
    /// use std::path::Path;
    ///
    /// let buffer = FileBuffer::new(Path::new("example.bin"))?;
    /// println!("File size: {} bytes", buffer.len());
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[must_use]
    pub fn len(&self) -> usize {
        self.mmap.len()
    }

    /// Returns true if the file is empty
    ///
    /// Note: This should never return true for a successfully created `FileBuffer`,
    /// as empty files are rejected during construction.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use libmagic_rs::io::FileBuffer;
    /// use std::path::Path;
    ///
    /// let buffer = FileBuffer::new(Path::new("example.bin"))?;
    /// assert!(!buffer.is_empty()); // Should always be false for valid buffers
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.mmap.is_empty()
    }
}

/// Safely reads bytes from a buffer with bounds checking
///
/// This function provides safe access to buffer data with comprehensive
/// bounds checking to prevent buffer overruns and invalid access patterns.
///
/// # Arguments
///
/// * `buffer` - The buffer to read from
/// * `offset` - Starting offset in the buffer
/// * `length` - Number of bytes to read
///
/// # Returns
///
/// Returns a slice of the requested bytes on success, or an `IoError` if
/// the access would be out of bounds.
///
/// # Errors
///
/// This function will return an error if:
/// - The offset is beyond the buffer size
/// - The length would cause an overflow
/// - The offset + length exceeds the buffer size
/// - The length is zero (invalid access)
///
/// # Examples
///
/// ```
/// use libmagic_rs::io::safe_read_bytes;
///
/// let buffer = b"Hello, World!";
/// let result = safe_read_bytes(buffer, 0, 5)?;
/// assert_eq!(result, b"Hello");
///
/// let result = safe_read_bytes(buffer, 7, 6)?;
/// assert_eq!(result, b"World!");
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn safe_read_bytes(
    buffer: &[u8],
    offset: BufferOffset,
    length: BufferLength,
) -> Result<&[u8], IoError> {
    buffer.get_safe_slice(offset, length)
}

/// Safely reads a single byte from a buffer with bounds checking
///
/// This is a convenience function for reading a single byte with proper
/// bounds checking.
///
/// # Arguments
///
/// * `buffer` - The buffer to read from
/// * `offset` - Offset of the byte to read
///
/// # Returns
///
/// Returns the byte at the specified offset on success, or an `IoError` if
/// the access would be out of bounds.
///
/// # Errors
///
/// This function will return an error if the offset is beyond the buffer size.
///
/// # Examples
///
/// ```
/// use libmagic_rs::io::safe_read_byte;
///
/// let buffer = b"Hello";
/// let byte = safe_read_byte(buffer, 0)?;
/// assert_eq!(byte, b'H');
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn safe_read_byte(buffer: &[u8], offset: BufferOffset) -> Result<u8, IoError> {
    buffer.get(offset).copied().ok_or(IoError::BufferOverrun {
        offset,
        length: 1,
        buffer_size: buffer.len(),
    })
}

/// Validates buffer access parameters without performing the actual read
///
/// This function can be used to validate buffer access parameters before
/// performing the actual read operation.
///
/// # Arguments
///
/// * `buffer_size` - Size of the buffer
/// * `offset` - Starting offset
/// * `length` - Number of bytes to access
///
/// # Returns
///
/// Returns `Ok(())` if the access is valid, or an `IoError` if it would
/// be out of bounds.
///
/// # Errors
///
/// This function will return an error if:
/// - The offset is beyond the buffer size
/// - The length would cause an overflow
/// - The offset + length exceeds the buffer size
/// - The length is zero (invalid access)
///
/// # Examples
///
/// ```
/// use libmagic_rs::io::validate_buffer_access;
///
/// // Valid access
/// validate_buffer_access(100, 10, 20)?;
///
/// // Invalid access - would go beyond buffer
/// let result = validate_buffer_access(100, 90, 20);
/// assert!(result.is_err());
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn validate_buffer_access(
    buffer_size: BufferLength,
    offset: BufferOffset,
    length: BufferLength,
) -> Result<(), IoError> {
    // Check for zero length (invalid access)
    if length == 0 {
        return Err(IoError::InvalidAccess { offset, length });
    }

    // Check if offset is within buffer bounds
    if offset >= buffer_size {
        return Err(IoError::BufferOverrun {
            offset,
            length,
            buffer_size,
        });
    }

    // Check for potential overflow in offset + length calculation
    let end_offset = offset
        .checked_add(length)
        .ok_or(IoError::InvalidAccess { offset, length })?;

    // Check if the end offset is within buffer bounds
    if end_offset > buffer_size {
        return Err(IoError::BufferOverrun {
            offset,
            length,
            buffer_size,
        });
    }

    Ok(())
}

// RAII cleanup is handled automatically by the Drop trait implementation
// of Mmap, which properly unmaps the memory and closes file handles.
// This implementation is kept explicit for documentation purposes.
impl Drop for FileBuffer {
    fn drop(&mut self) {
        // Mmap handles cleanup automatically through its Drop implementation
        // The memory mapping is safely unmapped and file handles are closed
        // No explicit cleanup needed here due to RAII patterns
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;

    /// Helper function to create a temporary file with given content
    fn create_temp_file(content: &[u8]) -> PathBuf {
        let temp_dir = std::env::temp_dir();
        let file_path = temp_dir.join(format!("test_file_{}", rand::random::<u32>()));

        let mut file = File::create(&file_path).expect("Failed to create temp file");
        file.write_all(content).expect("Failed to write temp file");
        file.sync_all().expect("Failed to sync temp file");

        file_path
    }

    /// Helper function to clean up temporary file
    fn cleanup_temp_file(path: &Path) {
        let _ = fs::remove_file(path);
    }

    #[test]
    fn test_file_buffer_creation_success() {
        let content = b"Hello, World!";
        let temp_path = create_temp_file(content);

        let buffer = FileBuffer::new(&temp_path).expect("Failed to create FileBuffer");

        assert_eq!(buffer.as_slice(), content);
        assert_eq!(buffer.len(), content.len());
        assert!(!buffer.is_empty());
        assert_eq!(buffer.path(), temp_path.as_path());

        cleanup_temp_file(&temp_path);
    }

    #[test]
    fn test_file_buffer_nonexistent_file() {
        let nonexistent_path = Path::new("/nonexistent/file.bin");

        let result = FileBuffer::new(nonexistent_path);

        assert!(result.is_err());
        match result.unwrap_err() {
            IoError::FileOpenError { path, .. } => {
                assert_eq!(path, nonexistent_path);
            }
            other => panic!("Expected FileOpenError, got {other:?}"),
        }
    }

    #[test]
    fn test_file_buffer_empty_file() {
        let temp_path = create_temp_file(&[]);

        let result = FileBuffer::new(&temp_path);

        assert!(result.is_err());
        match result.unwrap_err() {
            IoError::EmptyFile { path } => {
                assert_eq!(path, temp_path);
            }
            other => panic!("Expected EmptyFile error, got {other:?}"),
        }

        cleanup_temp_file(&temp_path);
    }

    #[test]
    fn test_file_buffer_large_file() {
        // Create a file with some content to test normal operation
        let content = vec![0u8; 1024]; // 1KB file
        let temp_path = create_temp_file(&content);

        let buffer =
            FileBuffer::new(&temp_path).expect("Failed to create FileBuffer for normal file");
        assert_eq!(buffer.len(), 1024);

        cleanup_temp_file(&temp_path);
    }

    #[test]
    fn test_file_buffer_binary_content() {
        let content = vec![0x00, 0x01, 0x02, 0x03, 0xFF, 0xFE, 0xFD, 0xFC];
        let temp_path = create_temp_file(&content);

        let buffer = FileBuffer::new(&temp_path).expect("Failed to create FileBuffer");

        assert_eq!(buffer.as_slice(), content.as_slice());
        assert_eq!(buffer.as_slice()[0], 0x00);
        assert_eq!(buffer.as_slice()[7], 0xFC);

        cleanup_temp_file(&temp_path);
    }

    #[test]
    fn test_io_error_display() {
        let path = PathBuf::from("/test/path");
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "File not found");

        let error = IoError::FileOpenError {
            path,
            source: io_err,
        };

        let error_string = format!("{error}");
        assert!(error_string.contains("/test/path"));
        assert!(error_string.contains("Failed to open file"));
    }

    #[test]
    fn test_empty_file_error_display() {
        let path = PathBuf::from("/test/empty.bin");
        let error = IoError::EmptyFile { path };

        let error_string = format!("{error}");
        assert!(error_string.contains("/test/empty.bin"));
        assert!(error_string.contains("is empty"));
    }

    #[test]
    fn test_file_too_large_error_display() {
        let path = PathBuf::from("/test/large.bin");
        let error = IoError::FileTooLarge {
            path,
            size: 2_000_000_000,
            max_size: 1_000_000_000,
        };

        let error_string = format!("{error}");
        assert!(error_string.contains("/test/large.bin"));
        assert!(error_string.contains("too large"));
        assert!(error_string.contains("2000000000"));
        assert!(error_string.contains("1000000000"));
    }

    #[test]
    fn test_safe_read_bytes_success() {
        let buffer = b"Hello, World!";

        // Read from beginning
        let result = safe_read_bytes(buffer, 0, 5).expect("Failed to read bytes");
        assert_eq!(result, b"Hello");

        // Read from middle
        let result = safe_read_bytes(buffer, 7, 5).expect("Failed to read bytes");
        assert_eq!(result, b"World");

        // Read single byte
        let result = safe_read_bytes(buffer, 0, 1).expect("Failed to read bytes");
        assert_eq!(result, b"H");

        // Read entire buffer
        let result = safe_read_bytes(buffer, 0, buffer.len()).expect("Failed to read bytes");
        assert_eq!(result, buffer);

        // Read from end
        let result = safe_read_bytes(buffer, buffer.len() - 1, 1).expect("Failed to read bytes");
        assert_eq!(result, b"!");
    }

    #[test]
    fn test_safe_read_bytes_out_of_bounds() {
        let buffer = b"Hello";

        // Offset beyond buffer
        let result = safe_read_bytes(buffer, 10, 1);
        assert!(result.is_err());
        match result.unwrap_err() {
            IoError::BufferOverrun {
                offset,
                length,
                buffer_size,
            } => {
                assert_eq!(offset, 10);
                assert_eq!(length, 1);
                assert_eq!(buffer_size, 5);
            }
            other => panic!("Expected BufferOverrun, got {other:?}"),
        }

        // Length extends beyond buffer
        let result = safe_read_bytes(buffer, 3, 5);
        assert!(result.is_err());
        match result.unwrap_err() {
            IoError::BufferOverrun {
                offset,
                length,
                buffer_size,
            } => {
                assert_eq!(offset, 3);
                assert_eq!(length, 5);
                assert_eq!(buffer_size, 5);
            }
            other => panic!("Expected BufferOverrun, got {other:?}"),
        }

        // Offset at buffer boundary
        let result = safe_read_bytes(buffer, 5, 1);
        assert!(result.is_err());
    }

    #[test]
    fn test_safe_read_bytes_zero_length() {
        let buffer = b"Hello";

        let result = safe_read_bytes(buffer, 0, 0);
        assert!(result.is_err());
        match result.unwrap_err() {
            IoError::InvalidAccess { offset, length } => {
                assert_eq!(offset, 0);
                assert_eq!(length, 0);
            }
            other => panic!("Expected InvalidAccess, got {other:?}"),
        }
    }

    #[test]
    fn test_safe_read_bytes_overflow() {
        let buffer = b"Hello";

        // Test potential overflow in offset + length
        // When offset is usize::MAX, it's beyond buffer bounds, so we get BufferOverrun
        let result = safe_read_bytes(buffer, usize::MAX, 1);
        assert!(result.is_err());
        match result.unwrap_err() {
            IoError::BufferOverrun { .. } => {
                // This is expected since usize::MAX > buffer.len()
            }
            other => panic!("Expected BufferOverrun, got {other:?}"),
        }

        // Test overflow with valid offset but huge length
        let result = safe_read_bytes(buffer, 1, usize::MAX);
        assert!(result.is_err());
        match result.unwrap_err() {
            IoError::InvalidAccess { .. } => {
                // This should trigger overflow in checked_add
            }
            other => panic!("Expected InvalidAccess, got {other:?}"),
        }

        // Test a case that would overflow but with smaller numbers
        let result = safe_read_bytes(buffer, 2, usize::MAX - 1);
        assert!(result.is_err());
        match result.unwrap_err() {
            IoError::InvalidAccess { .. } => {
                // This should trigger overflow in checked_add
            }
            other => panic!("Expected InvalidAccess, got {other:?}"),
        }
    }

    #[test]
    fn test_safe_read_byte_success() {
        let buffer = b"Hello";

        assert_eq!(safe_read_byte(buffer, 0).unwrap(), b'H');
        assert_eq!(safe_read_byte(buffer, 1).unwrap(), b'e');
        assert_eq!(safe_read_byte(buffer, 4).unwrap(), b'o');
    }

    #[test]
    fn test_safe_read_byte_out_of_bounds() {
        let buffer = b"Hello";

        let result = safe_read_byte(buffer, 5);
        assert!(result.is_err());
        match result.unwrap_err() {
            IoError::BufferOverrun {
                offset,
                length,
                buffer_size,
            } => {
                assert_eq!(offset, 5);
                assert_eq!(length, 1);
                assert_eq!(buffer_size, 5);
            }
            other => panic!("Expected BufferOverrun, got {other:?}"),
        }

        let result = safe_read_byte(buffer, 100);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_buffer_access_success() {
        // Valid accesses
        validate_buffer_access(100, 0, 50).expect("Should be valid");
        validate_buffer_access(100, 50, 50).expect("Should be valid");
        validate_buffer_access(100, 99, 1).expect("Should be valid");
        validate_buffer_access(10, 0, 10).expect("Should be valid");
        validate_buffer_access(1, 0, 1).expect("Should be valid");
    }

    #[test]
    fn test_validate_buffer_access_invalid() {
        // Zero length
        let result = validate_buffer_access(100, 0, 0);
        assert!(result.is_err());

        // Offset beyond buffer
        let result = validate_buffer_access(100, 100, 1);
        assert!(result.is_err());

        // Length extends beyond buffer
        let result = validate_buffer_access(100, 50, 51);
        assert!(result.is_err());

        // Overflow conditions
        let result = validate_buffer_access(100, usize::MAX, 1);
        assert!(result.is_err());

        let result = validate_buffer_access(100, 1, usize::MAX);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_buffer_access_edge_cases() {
        // Empty buffer
        let result = validate_buffer_access(0, 0, 1);
        assert!(result.is_err());

        // Large buffer, valid access
        let large_size = 1_000_000;
        validate_buffer_access(large_size, 0, large_size).expect("Should be valid");
        validate_buffer_access(large_size, large_size - 1, 1).expect("Should be valid");

        // Large buffer, invalid access
        let result = validate_buffer_access(large_size, large_size - 1, 2);
        assert!(result.is_err());
    }

    #[test]
    fn test_buffer_overrun_error_display() {
        let error = IoError::BufferOverrun {
            offset: 10,
            length: 5,
            buffer_size: 12,
        };

        let error_string = format!("{error}");
        assert!(error_string.contains("Buffer access out of bounds"));
        assert!(error_string.contains("offset 10"));
        assert!(error_string.contains("length 5"));
        assert!(error_string.contains("buffer size 12"));
    }

    #[test]
    fn test_invalid_access_error_display() {
        let error = IoError::InvalidAccess {
            offset: 0,
            length: 0,
        };

        let error_string = format!("{error}");
        assert!(error_string.contains("Invalid buffer access parameters"));
        assert!(error_string.contains("offset 0"));
        assert!(error_string.contains("length 0"));
    }
}

// Add a simple random number generator for tests to avoid external dependencies
#[cfg(test)]
mod rand {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[allow(clippy::cast_possible_truncation)]
    pub fn random<T: From<u32>>() -> T {
        let mut hasher = DefaultHasher::new();
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
            .hash(&mut hasher);
        T::from(hasher.finish() as u32)
    }
}
