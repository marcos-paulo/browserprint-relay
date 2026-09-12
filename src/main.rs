#![cfg_attr(windows, windows_subsystem = "windows")]

mod dialog;
mod icon;
mod net_info;
mod proxy;
mod settings;

use std::net::SocketAddr;
use std::thread;

use tao::event::Event;
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::TrayIconBuilder;

// Porta padrão do relay (usada só se não houver nenhuma salva no Registro
// ainda). Não pode ser a mesma do Browser Print (9100): no Windows, se o
// Browser Print já tiver aberto a porta com a flag SO_EXCLUSIVEADDRUSE
// (comum em serviços), nenhum outro processo consegue bindar aquele número
// de porta, nem em endereço diferente -- o bind falha com "acesso negado"
// (WSAEACCES / erro 10013), mesmo sem nenhum conflito de verdade. No Linux
// isso não acontece, mas o app é pra rodar no Windows, então evitamos a
// situação de vez usando uma porta própria.
const PORTA_PADRAO: u16 = 19100;
const BROWSER_PRINT_PORT: u16 = 9100;

#[derive(Debug, Clone)]
enum AppEvent {
    Status(proxy::Status),
}

fn main() -> anyhow::Result<()> {
    let event_loop = EventLoopBuilder::<AppEvent>::with_user_event().build();
    let event_proxy_servidor = event_loop.create_proxy();
    let event_proxy_menu = event_loop.create_proxy();

    let porta_inicial = settings::carregar_porta(PORTA_PADRAO);
    let ip_local = net_info::ip_local();

    let status_item = MenuItem::new("Status: iniciando…", false, None);
    let endereco_item = MenuItem::new("Endereço: detectando…", false, None);
    let copiar_item = MenuItem::new("Copiar endereço", ip_local.is_some(), None);
    let trocar_porta_item = MenuItem::new("Trocar porta…", true, None);
    let quit_item = MenuItem::new("Sair", true, None);

    let copiar_id = copiar_item.id().clone();
    let trocar_porta_id = trocar_porta_item.id().clone();
    let quit_id = quit_item.id().clone();

    let menu = Menu::new();
    menu.append(&status_item)?;
    menu.append(&endereco_item)?;
    menu.append(&copiar_item)?;
    menu.append(&trocar_porta_item)?;
    menu.append(&PredefinedMenuItem::separator())?;
    menu.append(&quit_item)?;

    let icon_ok = icon::make_icon(icon::Color::Green)?;
    let icon_err = icon::make_icon(icon::Color::Red)?;

    let tray = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_icon(icon_ok.clone())
        .with_tooltip("Zebra Browser Print Relay — iniciando…")
        .build()?;

    // Canal que carrega a porta atual pro servidor -- trocar o valor (feito
    // pelo menu "Trocar porta…") derruba o listener antigo e sobe um novo,
    // sem precisar reiniciar o app (ver proxy::run).
    let (port_tx, port_rx) = tokio::sync::watch::channel(porta_inicial);

    // Servidor roda numa thread separada, com seu próprio runtime tokio;
    // a thread principal fica livre pra bombear o loop de eventos da tray
    // (exigência do Win32: a mensagem da bandeja tem que rodar na UI thread).
    thread::spawn(move || {
        let target: SocketAddr = ([127, 0, 0, 1], BROWSER_PRINT_PORT).into();
        let rt = tokio::runtime::Runtime::new().expect("falha ao criar runtime tokio");
        rt.block_on(proxy::run(port_rx, target, move |status| {
            let _ = event_proxy_servidor.send_event(AppEvent::Status(status));
        }));
    });

    let menu_channel = MenuEvent::receiver();
    let mut endereco_atual = String::new();
    let mut porta_atual = porta_inicial;

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        if let Ok(menu_event) = menu_channel.try_recv() {
            if menu_event.id == quit_id {
                *control_flow = ControlFlow::Exit;
            } else if menu_event.id == copiar_id {
                if !endereco_atual.is_empty() {
                    if let Ok(mut clipboard) = arboard::Clipboard::new() {
                        let _ = clipboard.set_text(endereco_atual.clone());
                    }
                }
            } else if menu_event.id == trocar_porta_id {
                // PowerShell demora pra abrir -- roda numa thread à parte
                // pra não travar o loop de eventos da tray enquanto isso.
                let event_proxy = event_proxy_menu.clone();
                let port_tx = port_tx.clone();
                let porta_atual_snapshot = porta_atual;

                thread::spawn(move || {
                    let Some(texto) = dialog::pedir_texto(
                        "Browser Print Relay",
                        "Nova porta do relay (1-65535):",
                        &porta_atual_snapshot.to_string(),
                    ) else {
                        return;
                    };

                    let erro = match texto.trim().parse::<u16>() {
                        Ok(0) => Some("porta inválida: use um número entre 1 e 65535".to_string()),
                        Ok(nova_porta) if nova_porta == BROWSER_PRINT_PORT => Some(format!(
                            "porta inválida: {BROWSER_PRINT_PORT} é a porta do próprio Browser Print"
                        )),
                        Ok(nova_porta) => {
                            if let Err(e) = settings::salvar_porta(nova_porta) {
                                eprintln!("[browserprint-relay] falha ao salvar porta no registro: {e}");
                            }
                            let _ = port_tx.send(nova_porta);
                            None
                        }
                        Err(_) => Some("porta inválida: digite só números (1-65535)".to_string()),
                    };

                    if let Some(msg) = erro {
                        let _ = event_proxy.send_event(AppEvent::Status(proxy::Status::Error(msg)));
                    }
                });
            }
        }

        if let Event::UserEvent(AppEvent::Status(status)) = event {
            match status {
                proxy::Status::Listening(port) => {
                    porta_atual = port;
                    endereco_atual = match ip_local {
                        Some(ip) => format!("{ip}:{port}"),
                        None => String::new(),
                    };

                    let _ = tray.set_icon(Some(icon_ok.clone()));
                    let _ = tray.set_tooltip(Some(format!(
                        "Zebra Browser Print Relay — ouvindo em 0.0.0.0:{port}, repassando pra 127.0.0.1:{BROWSER_PRINT_PORT}"
                    )));
                    status_item.set_text(format!("Rodando — porta {port} -> {BROWSER_PRINT_PORT}"));
                    endereco_item.set_text(match ip_local {
                        Some(ip) => format!("Endereço: {ip}:{port}"),
                        None => "Endereço: não detectado (ver ipconfig)".to_string(),
                    });
                }
                proxy::Status::Error(msg) => {
                    let _ = tray.set_icon(Some(icon_err.clone()));
                    let _ = tray.set_tooltip(Some(format!("Erro: {msg}")));
                    status_item.set_text(format!("Erro: {msg}"));
                }
            }
        }
    });
}
