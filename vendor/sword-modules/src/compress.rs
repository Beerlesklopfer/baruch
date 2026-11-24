//! Decompressing z* blocks by `CompressType`.

use crate::conf::CompressType;
use crate::error::{Error, Result};

/// Fail early, when a module is opened, if its compression can't be read.
pub(crate) fn check_supported(kind: &CompressType) -> Result<()> {
    match kind {
        CompressType::Lzss => Ok(()),
        CompressType::Zip if cfg!(feature = "zip") => Ok(()),
        CompressType::Bzip2 if cfg!(feature = "bzip2") => Ok(()),
        CompressType::Xz if cfg!(feature = "xz") => Ok(()),
        CompressType::Other(name) => Err(Error::UnsupportedCompression(name.clone())),
        other => Err(Error::UnsupportedCompression(format!("{other:?} (cargo feature disabled)"))),
    }
}

/// Largest unpacked block (Baruch patch): a crafted module could otherwise unpack a few bytes into gigabytes
/// (decompression bomb) and end the process. Real blocks (a book with markup) stay far below.
pub const MAX_BLOCK_BYTES: usize = 64 << 20;

/// Refuse a block that unpacked to more than [`MAX_BLOCK_BYTES`] (read with one byte to spare).
#[cfg(any(feature = "zip", feature = "bzip2", feature = "xz"))]
fn bounded(name: &str, out: Vec<u8>) -> Result<Vec<u8>> {
    if out.len() > MAX_BLOCK_BYTES {
        return Err(Error::Decompress(format!("{name}: block unpacks to more than {} MiB", MAX_BLOCK_BYTES >> 20)));
    }
    Ok(out)
}

/// Unpack one block.
pub(crate) fn decompress(kind: &CompressType, data: &[u8]) -> Result<Vec<u8>> {
    match kind {
        CompressType::Lzss => Ok(crate::lzss::decode(data)),
        #[cfg(feature = "zip")]
        CompressType::Zip => {
            use std::io::Read;
            let mut out = Vec::new();
            flate2::read::ZlibDecoder::new(data)
                .take(MAX_BLOCK_BYTES as u64 + 1)
                .read_to_end(&mut out)
                .map_err(|e| Error::Decompress(format!("zlib: {e}")))?;
            bounded("zlib", out)
        }
        #[cfg(feature = "bzip2")]
        CompressType::Bzip2 => {
            use std::io::Read;
            let mut out = Vec::new();
            bzip2::read::BzDecoder::new(data)
                .take(MAX_BLOCK_BYTES as u64 + 1)
                .read_to_end(&mut out)
                .map_err(|e| Error::Decompress(format!("bzip2: {e}")))?;
            bounded("bzip2", out)
        }
        #[cfg(feature = "xz")]
        CompressType::Xz => {
            // A stream may be followed by zero padding; its footer never ends in a zero byte.
            let end = data.iter().rposition(|&b| b != 0).map_or(0, |i| i + 1);
            let mut input = &data[..end];
            let mut out = Limited { inner: Vec::new(), left: MAX_BLOCK_BYTES + 1 };
            match lzma_rs::xz_decompress(&mut input, &mut out) {
                Ok(()) => bounded("xz", out.inner),
                Err(_) if out.left == 0 => bounded("xz", out.inner),
                Err(e) => Err(Error::Decompress(format!("xz: {e}"))),
            }
        }
        other => {
            check_supported(other)?;
            Err(Error::UnsupportedCompression(format!("{other:?}")))
        }
    }
}

/// A writer that takes at most `left` bytes and then fails (Baruch patch): `lzma_rs` writes into any
/// `io::Write`, so this stops an xz bomb after [`MAX_BLOCK_BYTES`] + 1 bytes.
#[cfg(feature = "xz")]
struct Limited {
    inner: Vec<u8>,
    left: usize,
}

#[cfg(feature = "xz")]
impl std::io::Write for Limited {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if self.left == 0 {
            return Err(std::io::Error::other("block too large"));
        }
        let n = buf.len().min(self.left);
        self.inner.extend_from_slice(&buf[..n]);
        self.left -= n;
        Ok(n)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
