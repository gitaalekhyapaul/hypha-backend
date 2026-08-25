use std::io::{Read, Write};

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use super::VerificationWorkerProtocolError;

const FRAME_PREFIX_BYTES: usize = 4;

pub fn write_frame(
    writer: &mut impl Write,
    payload: &[u8],
    maximum: usize,
) -> Result<(), VerificationWorkerProtocolError> {
    if payload.is_empty() || payload.len() > maximum {
        return Err(VerificationWorkerProtocolError::FrameWrite);
    }
    let length =
        u32::try_from(payload.len()).map_err(|_| VerificationWorkerProtocolError::FrameWrite)?;
    writer
        .write_all(&length.to_be_bytes())
        .and_then(|()| writer.write_all(payload))
        .and_then(|()| writer.flush())
        .map_err(|_| VerificationWorkerProtocolError::FrameWrite)
}

pub fn read_frame_to_eof(
    reader: &mut impl Read,
    maximum: usize,
) -> Result<Vec<u8>, VerificationWorkerProtocolError> {
    let mut prefix = [0_u8; FRAME_PREFIX_BYTES];
    reader
        .read_exact(&mut prefix)
        .map_err(|_| VerificationWorkerProtocolError::FrameRead)?;
    let length = u32::from_be_bytes(prefix) as usize;
    if length == 0 || length > maximum {
        return Err(VerificationWorkerProtocolError::FrameRead);
    }
    let mut payload = vec![0_u8; length];
    reader
        .read_exact(&mut payload)
        .map_err(|_| VerificationWorkerProtocolError::FrameRead)?;
    let mut trailing = [0_u8; 1];
    match reader.read(&mut trailing) {
        Ok(0) => Ok(payload),
        Ok(_) => Err(VerificationWorkerProtocolError::TrailingFrameData),
        Err(_) => Err(VerificationWorkerProtocolError::FrameRead),
    }
}

pub(super) async fn write_frame_async(
    writer: &mut (impl AsyncWrite + Unpin),
    payload: &[u8],
    maximum: usize,
) -> Result<(), VerificationWorkerProtocolError> {
    if payload.is_empty() || payload.len() > maximum {
        return Err(VerificationWorkerProtocolError::FrameWrite);
    }
    let length =
        u32::try_from(payload.len()).map_err(|_| VerificationWorkerProtocolError::FrameWrite)?;
    writer
        .write_all(&length.to_be_bytes())
        .await
        .map_err(|_| VerificationWorkerProtocolError::FrameWrite)?;
    writer
        .write_all(payload)
        .await
        .map_err(|_| VerificationWorkerProtocolError::FrameWrite)?;
    writer
        .flush()
        .await
        .map_err(|_| VerificationWorkerProtocolError::FrameWrite)
}

pub(super) async fn read_frame_async(
    reader: &mut (impl AsyncRead + Unpin),
    maximum: usize,
) -> Result<Vec<u8>, VerificationWorkerProtocolError> {
    let mut prefix = [0_u8; FRAME_PREFIX_BYTES];
    reader
        .read_exact(&mut prefix)
        .await
        .map_err(|_| VerificationWorkerProtocolError::FrameRead)?;
    let length = u32::from_be_bytes(prefix) as usize;
    if length == 0 || length > maximum {
        return Err(VerificationWorkerProtocolError::FrameRead);
    }
    let mut payload = vec![0_u8; length];
    reader
        .read_exact(&mut payload)
        .await
        .map_err(|_| VerificationWorkerProtocolError::FrameRead)?;
    Ok(payload)
}

pub(super) async fn read_frame_to_eof_async(
    reader: &mut (impl AsyncRead + Unpin),
    maximum: usize,
) -> Result<Vec<u8>, VerificationWorkerProtocolError> {
    let payload = read_frame_async(reader, maximum).await?;
    let mut trailing = [0_u8; 1];
    match reader.read(&mut trailing).await {
        Ok(0) => Ok(payload),
        Ok(_) => Err(VerificationWorkerProtocolError::TrailingFrameData),
        Err(_) => Err(VerificationWorkerProtocolError::FrameRead),
    }
}
