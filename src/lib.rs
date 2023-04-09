//! Provides a simple cross-platform generic IPC server and client system built on top of the `interprocess` crate.
//!
//! Intended to be used for applications that will have a long-running 'server' process that can receive messages from 'client' processes, and may or may not reply back.
//!

mod errors;
pub use errors::*;

mod ext;
pub use ext::*;

mod utils;
use utils::current_process_instance_count;

use interprocess::local_socket::{LocalSocketListener, LocalSocketStream};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::io;
use std::thread::JoinHandle;

/// Attempts to spin up a thread that will listen for incoming connections on the given socket.
///
/// If the socket is already in use, it will check if there is more than one instance of the current process.
/// If there is, it will exit with an error.
///
/// It then creates a new thread where it will listen for incoming connections, and
/// invoke the passed `handle_connection` function.
///
/// # Arguments
///
/// * `socket` - The socket name to listen on.
/// * `handle_connection` - A function that will be invoked for each incoming connection.
/// * `handle_error` - An optional function that will be invoked if there is an error accepting a connection.
pub fn start_ipc_listener<F: Fn(LocalSocketStream) + Send + 'static>(
    socket: &str,
    on_connection: F,
    on_connection_error: Option<fn(io::Error)>,
) -> Result<JoinHandle<()>, IpcServerError> {
    let listener = match LocalSocketListener::bind(socket) {
        Err(e) if e.kind() == io::ErrorKind::AddrInUse => {
            if current_process_instance_count() > 1 {
                return Err(IpcServerError::AlreadyInUseError);
            }

            // The address was in use but there's no instances of this process running,
            // so it's likely a leftover socket file that we can delete.
            eprintln!("WARNING: Socket file already in use, deleting it and trying again.");

            std::fs::remove_file(socket).map_err(IpcServerError::FileError)?;
            LocalSocketListener::bind(socket).map_err(IpcServerError::BindError)?
        }
        Err(e) => return Err(IpcServerError::BindError(e)),
        Ok(listener) => listener,
    };

    let error_handler = move |inc: Result<LocalSocketStream, io::Error>| match inc {
        Ok(conn) => Some(conn),
        Err(e) => {
            if let Some(on_connection_error) = on_connection_error {
                on_connection_error(e);
            }
            None
        }
    };

    let thread = std::thread::Builder::new()
        .name(format!("ipc server '{socket}'"))
        .spawn(move || {
            for stream in listener.incoming().filter_map(error_handler) {
                on_connection(stream);
            }
        })
        .map_err(IpcServerError::ThreadSpawnError)?;

    Ok(thread)
}

/// A wrapper around `start_ipc_stream_listener`.
///
/// Rather than passing the LocalSocketStream directly to the `on_connection` callback,
/// this function instead reads a deserializable object from the socket and passes that, then optionally responds with a serializable object.
pub fn start_ipc_server<
    TRequest: DeserializeOwned,
    TResponse: Serialize,
    F: Fn(TRequest) -> Option<TResponse> + Send + 'static,
>(
    socket: &str,
    on_connection: F,
    on_connection_error: Option<fn(io::Error)>,
) -> Result<JoinHandle<()>, IpcServerError> {
    start_ipc_listener(
        socket,
        move |mut stream| {
            let request: TRequest = stream.read_serde().unwrap();

            if let Some(response) = on_connection(request) {
                stream.write_serde(&response).unwrap();
            }
        },
        on_connection_error,
    )
}

/// Connects to the socket and writes a serializable object to it.
/// Meant to be used for requests that don't expect a response from the server.
pub fn send_ipc_message<TRequest: Serialize>(
    socket_name: &str,
    request: TRequest,
) -> Result<(), IpcClientError> {
    let mut stream = LocalSocketStream::connect(socket_name)?;
    stream.write_serde(&request)?;
    Ok(())
}

/// Connect to the socket and write a serializable object to it, then immediately read a deserializable object from it,
/// blocking until a response is received. Meant to be used for requests that expect a response from the server.
pub fn send_ipc_query<TRequest: Serialize, TResponse: DeserializeOwned>(
    socket_name: &str,
    request: &TRequest,
) -> Result<TResponse, IpcClientError> {
    let mut stream = LocalSocketStream::connect(socket_name)?;
    stream.write_serde(&request)?;
    let response: TResponse = stream.read_serde()?;
    Ok(response)
}

/// Connects to the socket and returns the stream.
pub fn ipc_client_connect(socket_name: &str) -> Result<LocalSocketStream, IpcClientError> {
    LocalSocketStream::connect(socket_name).map_err(IpcClientError::ConnectError)
}
