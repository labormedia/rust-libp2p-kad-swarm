use async_trait::async_trait;
use libp2p::request_response::*;
use std::io;
use futures::{prelude::*, AsyncWriteExt};
use libp2p_core::upgrade::{
    read_length_prefixed,
    write_length_prefixed
};

#[derive(Debug, Clone)]
pub struct TestProtocol();
#[derive(Clone)]
pub struct TestCodec();
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SYN(Vec<u8>);
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SYNACK(Vec<u8>);
// #[derive(Debug, Clone, PartialEq, Eq)]
// struct ACK(Vec<u8>);

impl ProtocolName for TestProtocol {
    fn protocol_name(&self) -> &[u8] {
        "/SYNACK/0.0.1".as_bytes()
    }
}

#[async_trait]
impl RequestResponseCodec for TestCodec {
    type Protocol = TestProtocol;
    type Request = SYN;
    type Response = SYNACK;

    async fn read_request<T>(&mut self, _: &TestProtocol, io: &mut T) -> io::Result<Self::Request>
    where
        T: AsyncRead + Unpin + Send,
    {
        let vec = read_length_prefixed(io, 1024).await?;

        if vec.is_empty() {
            return Err(io::ErrorKind::UnexpectedEof.into());
        }

        Ok(SYN(vec))
    }

    async fn read_response<T>(&mut self, _: &TestProtocol, io: &mut T) -> io::Result<Self::Response>
    where
        T: AsyncRead + Unpin + Send,
    {
        let vec = read_length_prefixed(io, 1024).await?;

        if vec.is_empty() {
            return Err(io::ErrorKind::UnexpectedEof.into());
        }

        Ok(SYNACK(vec))
    }

    async fn write_request<T>(
        &mut self,
        _: &TestProtocol,
        io: &mut T,
        SYN(data): SYN,
    ) -> io::Result<()>
    where
        T: AsyncWrite + Unpin + Send,
    {
        write_length_prefixed(io, data).await?;
        io.close().await?;

        Ok(())
    }

    async fn write_response<T>(
        &mut self,
        _: &TestProtocol,
        io: &mut T,
        SYNACK(data): SYNACK,
    ) -> io::Result<()>
    where
        T: AsyncWrite + Unpin + Send,
    {
        write_length_prefixed(io, data).await?;
        io.close().await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    #[ignore]
    fn should_fail() {
        assert_eq!(1,2)
    }
}