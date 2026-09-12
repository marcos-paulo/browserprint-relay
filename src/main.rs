#![cfg_attr(windows, windows_subsystem = "windows")]

mod dialog;
mod icon;
mod net_info;
mod proxy;
mod settings;

use std::net::SocketAddr;
use std::thread;

use tao::event::Event;
use tao::event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy};
use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::TrayIconBuilder;

// Portas padrão do relay (usadas só se não houver nada salvo no Registro
// ainda). Não podem ser as mesmas do Browser Print (9100 HTTP / 9101
// HTTPS): no Windows, se o Browser Print já tiver aberto a porta com a
// flag SO_EXCLUSIVEADDRUSE (comum em serviços) -- ou, como descobrimos na
// prática, se ele já escuta em 0.0.0.0 na mesma porta --, nenhum outro
// processo consegue bindar aquele número, e o bind falha com "acesso
// negado" (WSAEACCES / erro 10013). Por isso o relay sempre usa portas
// próprias, só repassando pro destino certo.
const PORTA_HTTP_PADRAO: u16 = 19100;
const PORTA_HTTPS_PADRAO: u16 = 19101;

// Portas do Browser Print de verdade -- fixas, não configuráveis pelo menu,
// já que são a porta de outro programa, não do relay. HTTPS usa certificado
// autoassinado; como o relay só repassa bytes (não entende TLS), isso não
// muda nada pro relay -- é opaco tanto faz o conteúdo ser HTTP ou HTTPS.
const BROWSER_PRINT_HTTP_PORT: u16 = 9100;
const BROWSER_PRINT_HTTPS_PORT: u16 = 9101;
const PORTAS_BROWSER_PRINT: [u16; 2] = [BROWSER_PRINT_HTTP_PORT, BROWSER_PRINT_HTTPS_PORT];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Protocolo {
    Http,
    Https,
}

impl Protocolo {
    fn rotulo(self) -> &'static str {
        match self {
            Protocolo::Http => "HTTP",
            Protocolo::Https => "HTTPS",
        }
    }

    fn valor_registro(self) -> &'static str {
        match self {
            Protocolo::Http => settings::VALOR_PORTA_HTTP,
            Protocolo::Https => settings::VALOR_PORTA_HTTPS,
        }
    }
}

#[derive(Debug, Clone)]
enum AppEvent {
    Status(Protocolo, proxy::Status),
}

fn main() -> anyhow::Result<()> {
    let event_loop = EventLoopBuilder::<AppEvent>::with_user_event().build();
    let event_proxy_http = event_loop.create_proxy();
    let event_proxy_https = event_loop.create_proxy();
    let event_proxy_menu = event_loop.create_proxy();

    let porta_http_inicial =
        settings::carregar_porta(Protocolo::Http.valor_registro(), PORTA_HTTP_PADRAO);
    let porta_https_inicial =
        settings::carregar_porta(Protocolo::Https.valor_registro(), PORTA_HTTPS_PADRAO);
    let ip_local = net_info::ip_local();

    let status_http_item = MenuItem::new("HTTP: iniciando…", false, None);
    let endereco_http_item = MenuItem::new("Endereço HTTP: detectando…", false, None);
    let trocar_porta_http_item = MenuItem::new("Trocar porta HTTP…", true, None);

    let status_https_item = MenuItem::new("HTTPS: iniciando…", false, None);
    let endereco_https_item = MenuItem::new("Endereço HTTPS: detectando…", false, None);
    let trocar_porta_https_item = MenuItem::new("Trocar porta HTTPS…", true, None);

    let copiar_item = MenuItem::new("Copiar endereços", ip_local.is_some(), None);
    let quit_item = MenuItem::new("Sair", true, None);

    let trocar_porta_http_id = trocar_porta_http_item.id().clone();
    let trocar_porta_https_id = trocar_porta_https_item.id().clone();
    let copiar_id = copiar_item.id().clone();
    let quit_id = quit_item.id().clone();

    let menu = Menu::new();
    menu.append(&status_http_item)?;
    menu.append(&endereco_http_item)?;
    menu.append(&trocar_porta_http_item)?;
    menu.append(&PredefinedMenuItem::separator())?;
    menu.append(&status_https_item)?;
    menu.append(&endereco_https_item)?;
    menu.append(&trocar_porta_https_item)?;
    menu.append(&PredefinedMenuItem::separator())?;
    menu.append(&copiar_item)?;
    menu.append(&PredefinedMenuItem::separator())?;
    menu.append(&quit_item)?;

    // Ícone dividido ao meio: metade esquerda = HTTP, metade direita =
    // HTTPS. Os quatro estados possíveis (ok/ok, erro/erro, e os dois
    // parciais) ficam visíveis no próprio ícone da bandeja, sem precisar
    // abrir o menu pra saber qual dos dois está com problema.
    let icon_ambos_ok = icon::make_icon(icon::Color::Green)?;
    let icon_ambos_erro = icon::make_icon(icon::Color::Red)?;
    let icon_so_http_ok = icon::make_icon_dividido(icon::Color::Green, icon::Color::Red)?;
    let icon_so_https_ok = icon::make_icon_dividido(icon::Color::Red, icon::Color::Green)?;

    let tray = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_icon(icon_ambos_erro.clone())
        .with_tooltip("Zebra Browser Print Relay — iniciando…")
        .build()?;

    // Um canal de porta por protocolo -- trocar o valor (menu "Trocar porta
    // HTTP/HTTPS…") derruba o listener antigo daquele protocolo e sobe um
    // novo, sem precisar reiniciar o app nem afetar o outro protocolo (ver
    // proxy::run).
    let (port_tx_http, port_rx_http) = tokio::sync::watch::channel(porta_http_inicial);
    let (port_tx_https, port_rx_https) = tokio::sync::watch::channel(porta_https_inicial);

    // Os dois relays (HTTP e HTTPS) rodam na mesma thread/runtime, como duas
    // tarefas concorrentes -- não precisam de threads separadas, já que
    // nenhum dos dois bloqueia a outra (accept loop assíncrono). A thread
    // principal fica livre pra bombear o loop de eventos da tray (exigência
    // do Win32: mensagem da bandeja tem que rodar na UI thread).
    thread::spawn(move || {
        let target_http: SocketAddr = ([127, 0, 0, 1], BROWSER_PRINT_HTTP_PORT).into();
        let target_https: SocketAddr = ([127, 0, 0, 1], BROWSER_PRINT_HTTPS_PORT).into();
        let rt = tokio::runtime::Runtime::new().expect("falha ao criar runtime tokio");
        rt.block_on(async {
            tokio::join!(
                proxy::run(port_rx_http, target_http, move |status| {
                    let _ = event_proxy_http.send_event(AppEvent::Status(Protocolo::Http, status));
                }),
                proxy::run(port_rx_https, target_https, move |status| {
                    let _ =
                        event_proxy_https.send_event(AppEvent::Status(Protocolo::Https, status));
                }),
            );
        });
    });

    let menu_channel = MenuEvent::receiver();

    let mut ok_http = false;
    let mut ok_https = false;
    let mut porta_http_atual = porta_http_inicial;
    let mut porta_https_atual = porta_https_inicial;
    let mut endereco_http_atual = String::new();
    let mut endereco_https_atual = String::new();

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        if let Ok(menu_event) = menu_channel.try_recv() {
            if menu_event.id == quit_id {
                *control_flow = ControlFlow::Exit;
            } else if menu_event.id == copiar_id {
                let texto = format!("HTTP: {endereco_http_atual}\nHTTPS: {endereco_https_atual}");
                if !endereco_http_atual.is_empty() || !endereco_https_atual.is_empty() {
                    if let Ok(mut clipboard) = arboard::Clipboard::new() {
                        let _ = clipboard.set_text(texto);
                    }
                }
            } else if menu_event.id == trocar_porta_http_id {
                disparar_troca_de_porta(
                    Protocolo::Http,
                    porta_http_atual,
                    event_proxy_menu.clone(),
                    port_tx_http.clone(),
                );
            } else if menu_event.id == trocar_porta_https_id {
                disparar_troca_de_porta(
                    Protocolo::Https,
                    porta_https_atual,
                    event_proxy_menu.clone(),
                    port_tx_https.clone(),
                );
            }
        }

        if let Event::UserEvent(AppEvent::Status(protocolo, status)) = event {
            match status {
                proxy::Status::Listening(port) => {
                    let endereco = match ip_local {
                        Some(ip) => format!("{ip}:{port}"),
                        None => String::new(),
                    };
                    let texto_endereco = match ip_local {
                        Some(ip) => format!("Endereço {}: {ip}:{port}", protocolo.rotulo()),
                        None => format!("Endereço {}: não detectado (ver ipconfig)", protocolo.rotulo()),
                    };

                    match protocolo {
                        Protocolo::Http => {
                            ok_http = true;
                            porta_http_atual = port;
                            endereco_http_atual = endereco;
                            status_http_item.set_text(format!(
                                "HTTP: rodando — porta {port} -> {BROWSER_PRINT_HTTP_PORT}"
                            ));
                            endereco_http_item.set_text(texto_endereco);
                        }
                        Protocolo::Https => {
                            ok_https = true;
                            porta_https_atual = port;
                            endereco_https_atual = endereco;
                            status_https_item.set_text(format!(
                                "HTTPS: rodando — porta {port} -> {BROWSER_PRINT_HTTPS_PORT}"
                            ));
                            endereco_https_item.set_text(texto_endereco);
                        }
                    }
                }
                proxy::Status::Error(msg) => match protocolo {
                    Protocolo::Http => {
                        ok_http = false;
                        status_http_item.set_text(format!("HTTP: erro — {msg}"));
                    }
                    Protocolo::Https => {
                        ok_https = false;
                        status_https_item.set_text(format!("HTTPS: erro — {msg}"));
                    }
                },
            }

            let (icone, tooltip) = match (ok_http, ok_https) {
                (true, true) => (
                    &icon_ambos_ok,
                    format!(
                        "Browser Print Relay — HTTP :{porta_http_atual}->{BROWSER_PRINT_HTTP_PORT}, HTTPS :{porta_https_atual}->{BROWSER_PRINT_HTTPS_PORT}"
                    ),
                ),
                (true, false) => (
                    &icon_so_http_ok,
                    format!(
                        "Browser Print Relay — HTTP ok (:{porta_http_atual}->{BROWSER_PRINT_HTTP_PORT}), HTTPS com erro (ver menu)"
                    ),
                ),
                (false, true) => (
                    &icon_so_https_ok,
                    format!(
                        "Browser Print Relay — HTTPS ok (:{porta_https_atual}->{BROWSER_PRINT_HTTPS_PORT}), HTTP com erro (ver menu)"
                    ),
                ),
                (false, false) => (
                    &icon_ambos_erro,
                    "Browser Print Relay — HTTP e HTTPS com erro (ver menu)".to_string(),
                ),
            };
            let _ = tray.set_icon(Some(icone.clone()));
            let _ = tray.set_tooltip(Some(tooltip));
        }
    });
}

/// Pede a nova porta numa thread à parte (PowerShell demora pra abrir --
/// não pode travar o loop de eventos da tray) e, se válida, aplica na hora
/// (manda pro canal do protocolo, que derruba/sobe o listener) e salva no
/// Registro pra próxima vez.
fn disparar_troca_de_porta(
    protocolo: Protocolo,
    porta_atual: u16,
    event_proxy: EventLoopProxy<AppEvent>,
    port_tx: tokio::sync::watch::Sender<u16>,
) {
    thread::spawn(move || {
        let Some(texto) = dialog::pedir_texto(
            "Browser Print Relay",
            &format!("Nova porta do relay {} (1-65535):", protocolo.rotulo()),
            &porta_atual.to_string(),
        ) else {
            return;
        };

        let erro = match texto.trim().parse::<u16>() {
            Ok(0) => Some("porta inválida: use um número entre 1 e 65535".to_string()),
            Ok(nova_porta) if PORTAS_BROWSER_PRINT.contains(&nova_porta) => Some(format!(
                "porta inválida: {nova_porta} é uma porta do próprio Browser Print"
            )),
            Ok(nova_porta) => {
                if let Err(e) = settings::salvar_porta(protocolo.valor_registro(), nova_porta) {
                    eprintln!(
                        "[browserprint-relay] falha ao salvar porta {} no registro: {e}",
                        protocolo.rotulo()
                    );
                }
                let _ = port_tx.send(nova_porta);
                None
            }
            Err(_) => Some("porta inválida: digite só números (1-65535)".to_string()),
        };

        if let Some(msg) = erro {
            let _ = event_proxy.send_event(AppEvent::Status(protocolo, proxy::Status::Error(msg)));
        }
    });
}
