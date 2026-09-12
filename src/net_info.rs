use std::net::{IpAddr, UdpSocket};

/// Descobre o IP local que essa máquina usaria pra alcançar a rede (mesmo
/// endereço que aparece em `ipconfig`, na interface da rota padrão). Não
/// manda pacote nenhum de verdade: `UdpSocket::connect` só resolve a rota,
/// não estabelece conexão (UDP não tem handshake) -- é só um jeito barato de
/// perguntar pro sistema operacional "por qual interface eu sairia".
///
/// `8.8.8.8` é só o destino usado pra essa pergunta de roteamento; nada é
/// enviado pra ele.
pub fn ip_local() -> Option<IpAddr> {
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("8.8.8.8:80").ok()?;
    socket.local_addr().ok().map(|addr| addr.ip())
}
