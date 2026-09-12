use std::net::SocketAddr;

use tokio::net::{TcpListener, TcpStream};

#[derive(Debug, Clone)]
pub enum Status {
    Listening(u16),
    Error(String),
}

/// Escuta em 0.0.0.0:listen_port e repassa cada conexão pro Browser Print
/// real, rodando em `target` (normalmente 127.0.0.1:9100).
pub async fn run(listen_port: u16, target: SocketAddr, on_status: impl Fn(Status) + Send + 'static) {
    let listen_addr: SocketAddr = ([0, 0, 0, 0], listen_port).into();

    let listener = match TcpListener::bind(listen_addr).await {
        Ok(listener) => listener,
        Err(e) => {
            on_status(Status::Error(format!(
                "não consegui abrir a porta {listen_port}: {e}"
            )));
            return;
        }
    };

    on_status(Status::Listening(listen_port));

    loop {
        let (inbound, _peer) = match listener.accept().await {
            Ok(pair) => pair,
            Err(e) => {
                on_status(Status::Error(format!("falha ao aceitar conexão: {e}")));
                continue;
            }
        };

        tokio::spawn(async move {
            if let Err(e) = forward(inbound, target).await {
                eprintln!("erro no proxy: {e}");
            }
        });
    }
}

async fn forward(mut inbound: TcpStream, target: SocketAddr) -> std::io::Result<()> {
    let mut outbound = TcpStream::connect(target).await?;
    tokio::io::copy_bidirectional(&mut inbound, &mut outbound).await?;
    Ok(())
}
