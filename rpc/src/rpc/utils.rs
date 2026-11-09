use bytes::Bytes;
use ql_codec::Encode;

use crate::{
    codec, read_bytes, write_bytes, ChunkQueue, DecodeOwned, Error, RouterConfig, RpcError,
    RpcOptions, RpcRead, RpcWrite,
};

pub async fn write_eof_value<T, W>(mut writer: W, value: &T) -> Result<(), W::Error>
where
    T: Encode,
    W: RpcWrite,
{
    write_bytes(&mut writer, Bytes::from(value.encode_vec())).await?;
    writer.finish().await
}

pub async fn read_eof_value<T, R>(
    reader: &mut R,
    options: RpcOptions,
) -> Result<T, RpcError<R::Error>>
where
    T: DecodeOwned,
    R: RpcRead,
{
    let mut bytes = ChunkQueue::default();

    loop {
        let chunk = read_bytes(reader).await.map_err(RpcError::Transport)?;
        if chunk.is_empty() {
            break;
        }
        if chunk.len() > options.max_value_bytes - bytes.remaining() {
            reader.reset(ql_common::ResetCode::LIMIT);
            return Err(Error::LengthOverflow.into());
        }
        bytes.push(chunk);
    }

    codec::decode_exact(&mut bytes)
}

pub async fn read_framed_prefix<T, R>(
    reader: &mut R,
    options: RpcOptions,
) -> Result<(T, ChunkQueue), RpcError<R::Error>>
where
    T: DecodeOwned,
    R: RpcRead,
{
    let mut bytes = ChunkQueue::default();

    loop {
        if bytes
            .next_part_len()?
            .is_some_and(|len| len > options.max_value_bytes)
        {
            reader.reset(ql_common::ResetCode::LIMIT);
            return Err(Error::LengthOverflow.into());
        }
        let value = match bytes.try_take_part().map_err(RpcError::Protocol)? {
            Some(mut body) => Some(codec::decode_exact(&mut body)?),
            None => None,
        };
        if let Some(value) = value {
            return Ok((value, bytes));
        }

        let chunk = read_bytes(reader).await.map_err(RpcError::Transport)?;
        if chunk.is_empty() {
            return Err(Error::Truncated.into());
        }
        bytes.push(chunk);
    }
}

/// reads one eof-delimited value up to the configured request limit
pub async fn read_eof_request<T, R>(
    reader: &mut R,
    config: RouterConfig,
) -> Result<T, RpcError<R::Error>>
where
    T: DecodeOwned,
    R: RpcRead,
{
    let mut bytes = ChunkQueue::default();
    let mut total_read = 0usize;

    loop {
        let remaining = config.max_request_bytes.saturating_sub(total_read);
        let chunk = read_bytes(reader).await.map_err(RpcError::Transport)?;
        if chunk.is_empty() {
            break;
        }
        if chunk.len() > remaining {
            return Err(Error::LengthOverflow.into());
        }
        total_read += chunk.len();
        bytes.push(chunk);
    }

    codec::decode_exact(&mut bytes)
}
