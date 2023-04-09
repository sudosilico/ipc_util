# ipc-util

Provides simple cross-platform generic IPC message passing built on top of the `interprocess` crate.

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
ipc_util = "0.1"
```

## Usage

There are three functions that can be used to send messages to an IPC server as a client:

- The `send_ipc_message` function connects to the socket and sends an arbitrary serializable object over it.
- The `send_ipc_query` function connects to the socket, sends an arbitrary serializable object, and reads an arbitrary deserializable object in response.


There are two functions that can be used to spawn an IPC server thread:

- The `start_ipc_listener` function is used to spawn an IPC server thread using a callback that is passed a `LocalSocketStream` directly, as can be seen in the [stream example](examples/stream.rs).
- The `start_ipc_server` function is a wrapper around `start_ipc_listener`, where the callback instead receives an arbitrary serializable object `TRequest` and returns an `Option<TResponse>`. When a response is returned, it is sent back to the client. This can be seen in the [server example](examples/server.rs).

It's recommended to use `start_ipc_server` and its connection callback's `Option<T>` return type, where returning a `Some` variant will send that object as a response back to the client. This is intended to match up with the `send_ipc_query` function, which expects a response, while instances that return `None` to the callback will match up with `send_ipc_message` calls from clients.

## Example

```rust
use interprocess::local_socket::NameTypeSupport;
use ipc_util::{send_ipc_message, send_ipc_query, start_ipc_server};
use serde::{Deserialize, Serialize};

pub const MY_SOCKET_PATH: &str = "/tmp/ipc-util-ex-server.sock";
pub const MY_SOCKET_NAMESPACE: &str = "@ipc-util-ex-server.sock";

pub fn get_ipc_name() -> &'static str {
    use NameTypeSupport::*;
    match NameTypeSupport::query() {
        OnlyPaths => MY_SOCKET_PATH,
        OnlyNamespaced | Both => MY_SOCKET_NAMESPACE,
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Message {
    Text { text: String },
    Ping,
    Pong,
}

fn run_server() {
    start_ipc_server(
        get_ipc_name(),
        |message: Message| match message {
            Message::Text { text } => {
                println!("{text}");
                None
            }
            Message::Ping => Some(Message::Pong),
            _ => None,
        },
        Some(|e| panic!("Incoming connection error: {e}")),
    )
    .expect("Failed to start ipc listener")
    .join()
    .expect("Failed to join server thread");
}

fn run_client() {
    let text = Message::Text {
        text: "Hello from client!".to_string(),
    };

    let ping = Message::Ping;

    send_ipc_message(get_ipc_name(), &text).expect("Failed to connect to socket");

    let response: Message =
        send_ipc_query(get_ipc_name(), &ping).expect("Failed to connect to socket");

    dbg!(response);
}

fn main() {
    let args = std::env::args().collect::<Vec<_>>();

    match args.get(1).map(|s| s.as_str()) {
        Some("server") => run_server(),
        Some("client") => run_client(),
        _ => {
            println!("Usage: {} [server|client]", args[0]);
            std::process::exit(1);
        }
    }
}
```

You can run the examples with `cargo run --example server server` and `cargo run --example server client`.