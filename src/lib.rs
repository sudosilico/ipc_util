//! Provides simple cross-platform generic IPC message passing built on top of the `interprocess` crate.
//!
//! # Examples
//!
//! ```
//! use interprocess::local_socket::NameTypeSupport;
//! use ipc_util::{send_ipc_message, send_ipc_query, start_ipc_listener, SocketExt};
//! use serde::{Deserialize, Serialize};
//!
//! pub const MY_SOCKET_PATH: &str = "/tmp/ipc-util-ex-stream.sock";
//! pub const MY_SOCKET_NAMESPACE: &str = "@ipc-util-ex-stream.sock";
//!
//! pub fn get_ipc_name() -> &'static str {
//!     use NameTypeSupport::*;
//!     match NameTypeSupport::query() {
//!         OnlyPaths => MY_SOCKET_PATH,
//!         OnlyNamespaced | Both => MY_SOCKET_NAMESPACE,
//!     }
//! }
//!
//! #[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
//! pub enum Message {
//!     Text { text: String },
//!     Ping,
//!     Pong,
//! }
//!
//! fn run_server() {
//!     start_ipc_listener(
//!         get_ipc_name(),
//!         |mut stream| {
//!             // Read message from client
//!             let message: Message = stream.read_serde().expect("Failed to read message");
//!
//!             // Handle message
//!             match message {
//!                 Message::Text { text } => {
//!                     println!("{text}");
//!                 }
//!                 Message::Ping => {
//!                     stream
//!                         .write_serde(&Message::Pong)
//!                         .expect("Failed to write pong");
//!                 }
//!                 _ => {}
//!             };
//!         },
//!         None,
//!     )
//!     .expect("Failed to bind to socket")
//!     .join()
//!     .expect("Failed to join server thread");
//! }
//!
//! fn run_client() {
//!     let text = Message::Text {
//!         text: "Hello from client!".to_string(),
//!     };
//!
//!     let ping = Message::Ping;
//!
//!     send_ipc_message(get_ipc_name(), &text).expect("Failed to connect to socket");
//!
//!     let response: Message =
//!         send_ipc_query(get_ipc_name(), &ping).expect("Failed to connect to socket");
//!
//!     dbg!(response);
//! }
//!
//! fn main() {
//!     let args = std::env::args().collect::<Vec<_>>();
//!
//!     match args.get(1).map(|s| s.as_str()) {
//!         Some("server") => run_server(),
//!         Some("client") => run_client(),
//!         _ => {
//!             println!("Usage: {} [server|client]", args[0]);
//!             std::process::exit(1);
//!         }
//!     }
//! }
//! ```

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

/// A wrapper around `start_ipc_listener`.
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
    request: &TRequest,
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
