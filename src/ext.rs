use crate::{IpcStreamReadError, IpcStreamWriteError};
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use interprocess::local_socket::LocalSocketStream;
use std::io::prelude::*;

pub trait SocketExt {
    fn read_serde<T: serde::de::DeserializeOwned>(&mut self) -> Result<T, IpcStreamReadError>;
    fn write_serde<T: serde::Serialize>(&mut self, data: &T) -> Result<(), IpcStreamWriteError>;
}

impl SocketExt for LocalSocketStream {
    /// Read a serializable object from the socket.
    ///
    /// This reads a `u32` in little endian, then reads that many bytes from the socket, then deserializes the data using `bincode::deserialize`.
    fn read_serde<T: serde::de::DeserializeOwned>(&mut self) -> Result<T, IpcStreamReadError> {
        let size = self.read_u32::<LittleEndian>()?;

        let bytes = {
            let mut bytes = vec![0; size as usize];

            self.read_exact(&mut bytes)?;

            bytes
        };

        let result: T = bincode::deserialize(&bytes)?;

        Ok(result)
    }

    /// Write a serializable object to the socket.
    ///
    /// This serializes the data using `bincode::serialize`, writes the length of the serialized data as a `u32` in little endian, then writes the serialized data.
    fn write_serde<T: serde::Serialize>(&mut self, data: &T) -> Result<(), IpcStreamWriteError> {
        let bytes = bincode::serialize(data)?;

        self.write_u32::<LittleEndian>(bytes.len() as u32)?;
        self.write_all(&bytes)?;

        Ok(())
    }
}
