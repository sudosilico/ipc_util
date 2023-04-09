# ipc-util

Provides simple cross-platform generic IPC message passing built on top of the `interprocess` crate.

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
ipc_util = "0.1"
```

## Usage

Define your socket paths and a function that returns the correct path for the current platform:

```rust
use ipc_util::*;
use interprocess::local_socket::NameTypeSupport;

pub const MY_SOCKET_PATH: &str = "/tmp/my-socket.sock";
pub const MY_SOCKET_NAMESPACE: &str = "@my-socket.sock";

pub fn get_ipc_name() -> &'static str {
    use NameTypeSupport::*;
    match NameTypeSupport::query() {
        OnlyPaths => MY_SOCKET_PATH,
        OnlyNamespaced | Both => MY_SOCKET_NAMESPACE,
    }
}
```

Make sure the type that you want to send over the socket is properly de/serializable:

```rust
#[derive(Serialize, Deserialize)]
pub enum Message {
    Text { text: String },
    Ping,
    Pong,
}
```

There are three functions that can be used to send messages to an IPC server as a client:

- The `send_ipc_message` function connects to the socket and sends an arbitrary serializable object over it.
- The `send_ipc_query` function connects to the socket, sends an arbitrary serializable object, and reads an arbitrary deserializable object in response.


There are two functions that can be used to spawn an IPC server thread:

- The `start_ipc_listener` function is used to spawn an IPC server thread using a callback that is passed a `LocalSocketStream` directly, as can be seen in the [stream example](examples/stream.rs).
- The `start_ipc_server` function is a wrapper around `start_ipc_listener`, where the callback instead receives an arbitrary serializable object `TRequest` and returns an `Option<TResponse>`. When a response is returned, it is sent back to the client. This can be seen in the [server example](examples/server.rs).

It's recommended to use `start_ipc_server` and its connection callback's `Option<T>` return type, where returning a `Some` variant will send that object as a response back to the client. This is intended to match up with the `send_ipc_query` function, which expects a response, while instances that return `None` to the callback will match up with `send_ipc_message` calls from clients.