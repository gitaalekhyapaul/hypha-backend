use std::io::{Read, Write};

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use super::OneShotProtocolError;

const FRAME_PREFIX_BYTES: usize = 4;

/// Writes one big-endian length-prefixed frame without closing the channel.
pub fn write_frame(
    writer: &mut impl Write,
    payload: &[u8],
    maximum: usize,
) -> Result<(), OneShotProtocolError> {
    if payload.is_empty() || payload.len() > maximum {
        return Err(OneShotProtocolError::FrameWrite);
    }
    let length = u32::try_from(payload.len()).map_err(|_| OneShotProtocolError::FrameWrite)?;
    writer
        .write_all(&length.to_be_bytes())
        .and_then(|()| writer.write_all(payload))
        .and_then(|()| writer.flush())
        .map_err(|_| OneShotProtocolError::FrameWrite)
}

/// Reads exactly one bounded frame and requires EOF after its payload.
pub fn read_frame_to_eof(
    reader: &mut impl Read,
    maximum: usize,
) -> Result<Vec<u8>, OneShotProtocolError> {
    let mut prefix = [0_u8; FRAME_PREFIX_BYTES];
    reader
        .read_exact(&mut prefix)
        .map_err(|_| OneShotProtocolError::FrameRead)?;
    let length = u32::from_be_bytes(prefix) as usize;
    if length == 0 || length > maximum {
        return Err(OneShotProtocolError::FrameRead);
    }
    let mut payload = vec![0_u8; length];
    reader
        .read_exact(&mut payload)
        .map_err(|_| OneShotProtocolError::FrameRead)?;
    let mut trailing = [0_u8; 1];
    match reader.read(&mut trailing) {
        Ok(0) => Ok(payload),
        Ok(_) => Err(OneShotProtocolError::TrailingFrameData),
        Err(_) => Err(OneShotProtocolError::FrameRead),
    }
}

pub(crate) async fn write_frame_async(
    writer: &mut (impl AsyncWrite + Unpin),
    payload: &[u8],
    maximum: usize,
) -> Result<(), OneShotProtocolError> {
    if payload.is_empty() || payload.len() > maximum {
        return Err(OneShotProtocolError::FrameWrite);
    }
    let length = u32::try_from(payload.len()).map_err(|_| OneShotProtocolError::FrameWrite)?;
    writer
        .write_all(&length.to_be_bytes())
        .await
        .map_err(|_| OneShotProtocolError::FrameWrite)?;
    writer
        .write_all(payload)
        .await
        .map_err(|_| OneShotProtocolError::FrameWrite)?;
    writer
        .flush()
        .await
        .map_err(|_| OneShotProtocolError::FrameWrite)
}

pub(crate) async fn read_frame_async(
    reader: &mut (impl AsyncRead + Unpin),
    maximum: usize,
) -> Result<Vec<u8>, OneShotProtocolError> {
    let mut prefix = [0_u8; FRAME_PREFIX_BYTES];
    reader
        .read_exact(&mut prefix)
        .await
        .map_err(|_| OneShotProtocolError::FrameRead)?;
    let length = u32::from_be_bytes(prefix) as usize;
    if length == 0 || length > maximum {
        return Err(OneShotProtocolError::FrameRead);
    }
    let mut payload = vec![0_u8; length];
    reader
        .read_exact(&mut payload)
        .await
        .map_err(|_| OneShotProtocolError::FrameRead)?;
    Ok(payload)
}

pub(crate) async fn read_frame_to_eof_async(
    reader: &mut (impl AsyncRead + Unpin),
    maximum: usize,
) -> Result<Vec<u8>, OneShotProtocolError> {
    let payload = read_frame_async(reader, maximum).await?;
    let mut trailing = [0_u8; 1];
    match reader.read(&mut trailing).await {
        Ok(0) => Ok(payload),
        Ok(_) => Err(OneShotProtocolError::TrailingFrameData),
        Err(_) => Err(OneShotProtocolError::FrameRead),
    }
}
