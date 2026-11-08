use md5::Md5;
use serde::{Deserialize, Serialize};
use sha1::Sha1;
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{self, BufReader, Read};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffRange {
    pub start: u64,
    pub len: u64,
    pub is_near_boundary: bool,
    pub boundary_note: String,
}

#[derive(Debug, Clone)]
pub struct VerificationReport {
    pub file_a_path: String,
    pub file_b_path: String,
    pub file_a_size: u64,
    pub file_b_size: u64,
    pub file_a_sha256: String,
    pub file_b_sha256: String,
    pub is_identical: bool,
    pub total_diff_bytes: u64,
    pub total_diff_ranges: usize,
    pub diff_ranges: Vec<DiffRange>, // capped at 100
}

impl VerificationReport {
    pub fn format_summary(&self) -> String {
        let mut out = String::new();
        out.push_str("================ FILE INTEGRITY VERIFICATION REPORT ================\n");
        out.push_str(&format!("File A: {}\n", self.file_a_path));
        out.push_str(&format!("File B: {}\n", self.file_b_path));
        out.push_str(&format!("File A Size:   {} bytes\n", self.file_a_size));
        out.push_str(&format!("File B Size:   {} bytes\n", self.file_b_size));
        out.push_str(&format!("File A SHA256: {}\n", self.file_a_sha256));
        out.push_str(&format!("File B SHA256: {}\n", self.file_b_sha256));
        out.push_str(&format!("Identical:     {}\n", if self.is_identical { "YES (100% Match)" } else { "NO (Corrupted / Mismatched)" }));

        if !self.is_identical {
            out.push_str(&format!("Total differing bytes:  {}\n", self.total_diff_bytes));
            out.push_str(&format!("Total differing ranges: {}\n", self.total_diff_ranges));
            out.push_str(&format!("Showing first {} differing ranges:\n", self.diff_ranges.len()));
            for (idx, r) in self.diff_ranges.iter().enumerate() {
                out.push_str(&format!(
                    "  [{:>3}] Offset: 0x{:08X} ({:>10}) .. 0x{:08X} ({:>10}) | Len: {:>8} bytes | Boundary: {}\n",
                    idx + 1,
                    r.start,
                    r.start,
                    r.start + r.len - 1,
                    r.start + r.len - 1,
                    r.len,
                    r.boundary_note
                ));
            }
        }
        out.push_str("====================================================================\n");
        out
    }
}

pub fn is_near_boundary(offset: u64, chunk_size: Option<u64>, known_boundaries: &[u64]) -> (bool, String) {
    const BOUNDARY_TOLERANCE: u64 = 16;

    for &kb in known_boundaries {
        let diff = offset.abs_diff(kb);
        if diff <= BOUNDARY_TOLERANCE {
            return (true, format!("within {}B of known boundary {}", diff, kb));
        }
    }

    if let Some(cs) = chunk_size {
        if cs > 0 {
            let rem = offset % cs;
            if rem <= BOUNDARY_TOLERANCE {
                return (true, format!("within {}B past chunk multiple of {}", rem, cs));
            }
            let dist_to_next = cs - rem;
            if dist_to_next <= BOUNDARY_TOLERANCE {
                return (true, format!("within {}B before chunk multiple of {}", dist_to_next, cs));
            }
        }
    }

    (false, "internal (no boundary proximity)".to_string())
}

pub fn compare_files(
    path_a: &Path,
    path_b: &Path,
    chunk_size: Option<u64>,
    known_boundaries: &[u64],
) -> io::Result<VerificationReport> {
    let mut file_a = BufReader::new(File::open(path_a)?);
    let mut file_b = BufReader::new(File::open(path_b)?);

    let meta_a = path_a.metadata()?;
    let meta_b = path_b.metadata()?;
    let size_a = meta_a.len();
    let size_b = meta_b.len();

    let mut hasher_a = Sha256::new();
    let mut hasher_b = Sha256::new();

    let mut diff_ranges = Vec::new();
    let mut total_diff_ranges = 0usize;
    let mut total_diff_bytes = 0u64;

    const BUFFER_SIZE: usize = 64 * 1024; // 64 KiB
    let mut buf_a = vec![0u8; BUFFER_SIZE];
    let mut buf_b = vec![0u8; BUFFER_SIZE];

    let mut current_offset = 0u64;
    let mut active_diff_start: Option<u64> = None;

    loop {
        let n_a = file_a.read(&mut buf_a)?;
        let n_b = file_b.read(&mut buf_b)?;

        if n_a > 0 {
            hasher_a.update(&buf_a[..n_a]);
        }
        if n_b > 0 {
            hasher_b.update(&buf_b[..n_b]);
        }

        let common = n_a.min(n_b);
        for i in 0..common {
            let byte_offset = current_offset + i as u64;
            if buf_a[i] != buf_b[i] {
                total_diff_bytes += 1;
                if active_diff_start.is_none() {
                    active_diff_start = Some(byte_offset);
                }
            } else if let Some(start) = active_diff_start.take() {
                total_diff_ranges += 1;
                if diff_ranges.len() < 100 {
                    let len = byte_offset - start;
                    let (near_start, note_start) = is_near_boundary(start, chunk_size, known_boundaries);
                    let (near_end, note_end) = is_near_boundary(byte_offset, chunk_size, known_boundaries);
                    let is_near = near_start || near_end;
                    let note = if near_start {
                        format!("start {}", note_start)
                    } else if near_end {
                        format!("end {}", note_end)
                    } else {
                        note_start
                    };
                    diff_ranges.push(DiffRange {
                        start,
                        len,
                        is_near_boundary: is_near,
                        boundary_note: note,
                    });
                }
            }
        }

        if n_a != n_b {
            let max_n = n_a.max(n_b);
            let excess = (max_n - common) as u64;
            total_diff_bytes += excess;
            if active_diff_start.is_none() {
                active_diff_start = Some(current_offset + common as u64);
            }
        }

        current_offset += common as u64;
        if n_a == 0 && n_b == 0 {
            break;
        }
    }

    if let Some(start) = active_diff_start.take() {
        total_diff_ranges += 1;
        let max_len = size_a.max(size_b);
        let len = max_len.saturating_sub(start);
        if diff_ranges.len() < 100 {
            let (near_start, note_start) = is_near_boundary(start, chunk_size, known_boundaries);
            diff_ranges.push(DiffRange {
                start,
                len,
                is_near_boundary: near_start,
                boundary_note: format!("tail {}", note_start),
            });
        }
    }

    let sha_a = hex::encode(hasher_a.finalize());
    let sha_b = hex::encode(hasher_b.finalize());
    let is_identical = size_a == size_b && sha_a == sha_b && total_diff_ranges == 0;

    Ok(VerificationReport {
        file_a_path: path_a.to_string_lossy().to_string(),
        file_b_path: path_b.to_string_lossy().to_string(),
        file_a_size: size_a,
        file_b_size: size_b,
        file_a_sha256: sha_a,
        file_b_sha256: sha_b,
        is_identical,
        total_diff_bytes,
        total_diff_ranges,
        diff_ranges,
    })
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FileChecksums {
    pub crc32: String,
    pub md5: String,
    pub sha1: String,
    pub sha256: String,
    pub file_size: u64,
}

/// Zero-copy, high-performance streaming multi-hasher.
/// Calculates CRC32, MD5, SHA-1, and SHA-256 simultaneously in a single IO pass.
pub struct MultiHasher {
    crc: u32,
    md5: Md5,
    sha1: Sha1,
    sha256: Sha256,
    total_bytes: u64,
}

impl Default for MultiHasher {
    fn default() -> Self {
        Self::new()
    }
}

impl MultiHasher {
    pub fn new() -> Self {
        Self {
            crc: 0xFFFF_FFFF,
            md5: Md5::new(),
            sha1: Sha1::new(),
            sha256: Sha256::new(),
            total_bytes: 0,
        }
    }

    pub fn update(&mut self, chunk: &[u8]) {
        self.crc = update_crc32(self.crc, chunk);
        self.md5.update(chunk);
        self.sha1.update(chunk);
        self.sha256.update(chunk);
        self.total_bytes += chunk.len() as u64;
    }

    pub fn finalize(self) -> FileChecksums {
        let final_crc = !self.crc;
        FileChecksums {
            crc32: format!("{:08X}", final_crc),
            md5: hex::encode(self.md5.finalize()),
            sha1: hex::encode(self.sha1.finalize()),
            sha256: hex::encode(self.sha256.finalize()),
            file_size: self.total_bytes,
        }
    }

    pub fn compute_file(path: &Path) -> io::Result<FileChecksums> {
        let mut file = BufReader::new(File::open(path)?);
        let mut hasher = Self::new();
        let mut buf = vec![0u8; 64 * 1024];
        loop {
            let n = file.read(&mut buf)?;
            if n == 0 {
                break;
            }
            hasher.update(&buf[..n]);
        }
        Ok(hasher.finalize())
    }
}

fn update_crc32(mut crc: u32, bytes: &[u8]) -> u32 {
    const TABLE: [u32; 256] = generate_crc32_table();
    for &b in bytes {
        let idx = ((crc ^ (b as u32)) & 0xFF) as usize;
        crc = (crc >> 8) ^ TABLE[idx];
    }
    crc
}

const fn generate_crc32_table() -> [u32; 256] {
    let mut table = [0u32; 256];
    let mut i = 0usize;
    while i < 256 {
        let mut crc = i as u32;
        let mut j = 0;
        while j < 8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ 0xEDB88320;
            } else {
                crc >>= 1;
            }
            j += 1;
        }
        table[i] = crc;
        i += 1;
    }
    table
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_multihasher_standard_vector() {
        let mut hasher = MultiHasher::new();
        hasher.update(b"123456789");
        let sums = hasher.finalize();

        assert_eq!(sums.crc32, "CBF43926");
        assert_eq!(sums.md5, "25f9e794323b453885f5181f1b624d0b");
        assert_eq!(sums.sha1, "f7c3bc1d808e04732adf679965ccc34ca7ae3441");
        assert_eq!(
            sums.sha256,
            "15e2b0d3c33891ebb0f1ef609ec419420c20e320ce94c65fbc8c3312448eb225"
        );
        assert_eq!(sums.file_size, 9);
    }
}
