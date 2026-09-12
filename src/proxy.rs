use std::net::SocketAddr;

use tokio::net::{TcpListener, TcpStream};
use tokio::sync::watch;

#[derive(Debug, Clone)]
pub enum Status {
    Listening(u16),
    Error(String),
}

/// Escuta em 0.0.0.0:<porta atual> e repassa cada conexão pro Browser Print
/// real, rodando em `target` (normalmente 127.0.0.1:9100). A porta ouvida
/// pode mudar em tempo real através de `port_rx` (ver troca de porta no
/// menu da bandeja, em main.rs) -- cada mudança derruba o listener antigo
/// (liberando a porta) e sobe um novo, sem precisar reiniciar o app.
pub async fn run(
    mut port_rx: watch::Receiver<u16>,
    target: SocketAddr,
    on_status: impl Fn(Status) + Send + Sync + 'static,
) {
    loop {
        let port = *port_rx.borrow_and_update();

        // `servir` só devolve o controle quando `port_rx` muda -- nunca por
        // conta própria (nem em erro de bind, ver abaixo) -- então o select!
        // sempre resolve pelo lado de `changed()`, cancelando e descartando
        // o listener em andamento (junto com o `TcpListener`, fechando a
        // porta) antes de voltar ao topo do loop com a porta nova.
        tokio::select! {
            _ = servir(port, target, &on_status) => {}
            _ = port_rx.changed() => {}
        }
    }
}

async fn servir(port: u16, target: SocketAddr, on_status: &(impl Fn(Status) + Send + Sync)) {
    let listen_addr: SocketAddr = ([0, 0, 0, 0], port).into();

    let listener = match TcpListener::bind(listen_addr).await {
        Ok(listener) => listener,
        Err(e) => {
            on_status(Status::Error(format!("não consegui abrir a porta {port}: {e}")));
            // Fica parado (sem tentar de novo sozinho) até o usuário trocar a
            // porta pelo menu -- tentar de novo em loop só bateria no mesmo
            // erro repetidamente.
            std::future::pending::<()>().await;
            return;
        }
    };

    on_status(Status::Listening(port));

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
