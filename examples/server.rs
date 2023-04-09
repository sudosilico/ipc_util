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

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ipc_communication() {
        // Start server in a separate thread
        std::thread::spawn(move || {
            run_server();
        });

        // Wait for server to start
        std::thread::sleep(std::time::Duration::from_millis(100));

        // Send message from client
        let text = Message::Text {
            text: "Hello from client!".to_string(),
        };
        send_ipc_message(get_ipc_name(), &text).expect("Failed to connect to socket");

        // Send query from client
        let ping = Message::Ping;
        let response: Message =
            send_ipc_query(get_ipc_name(), &ping).expect("Failed to connect to socket");

        // Check response
        assert_eq!(response, Message::Pong);
    }
}
