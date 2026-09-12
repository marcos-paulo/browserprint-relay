#![cfg_attr(windows, windows_subsystem = "windows")]

mod icon;
mod proxy;

use std::net::SocketAddr;
use std::thread;

use tao::event::Event;
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::TrayIconBuilder;

// Porta que o relay escuta pra rede. Não pode ser a mesma do Browser Print
// (9100): no Windows, se o Browser Print já tiver aberto a porta com a
// flag SO_EXCLUSIVEADDRUSE (comum em serviços), nenhum outro processo
// consegue bindar aquele número de porta, nem em endereço diferente --
// o bind falha com "acesso negado" (WSAEACCES / erro 10013), mesmo sem
// nenhum conflito de verdade. No Linux isso não acontece (endereços
// diferentes no mesmo número de porta convivem sem problema), mas o app
// é pra rodar no Windows, então evitamos a situação de vez usando uma
// porta própria.
const RELAY_PORT: u16 = 19100;
const BROWSER_PRINT_PORT: u16 = 9100;

#[derive(Debug, Clone)]
enum AppEvent {
    Status(proxy::Status),
}

fn main() -> anyhow::Result<()> {
    let event_loop = EventLoopBuilder::<AppEvent>::with_user_event().build();
    let event_proxy = event_loop.create_proxy();

    let status_item = MenuItem::new("Status: iniciando…", false, None);
    let quit_item = MenuItem::new("Sair", true, None);
    let quit_id = quit_item.id().clone();

    let menu = Menu::new();
    menu.append(&status_item)?;
    menu.append(&PredefinedMenuItem::separator())?;
    menu.append(&quit_item)?;

    let icon_ok = icon::make_icon(icon::Color::Green)?;
    let icon_err = icon::make_icon(icon::Color::Red)?;

    let tray = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_icon(icon_ok.clone())
        .with_tooltip("Zebra Browser Print Relay — iniciando…")
        .build()?;

    // Servidor roda numa thread separada, com seu próprio runtime tokio;
    // a thread principal fica livre pra bombear o loop de eventos da tray
    // (exigência do Win32: a mensagem da bandeja tem que rodar na UI thread).
    thread::spawn(move || {
        let target: SocketAddr = ([127, 0, 0, 1], BROWSER_PRINT_PORT).into();
        let rt = tokio::runtime::Runtime::new().expect("falha ao criar runtime tokio");
        rt.block_on(proxy::run(RELAY_PORT, target, move |status| {
            let _ = event_proxy.send_event(AppEvent::Status(status));
        }));
    });

    let menu_channel = MenuEvent::receiver();

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        if let Ok(menu_event) = menu_channel.try_recv() {
            if menu_event.id == quit_id {
                *control_flow = ControlFlow::Exit;
            }
        }

        if let Event::UserEvent(AppEvent::Status(status)) = event {
            match status {
                proxy::Status::Listening(port) => {
                    let _ = tray.set_icon(Some(icon_ok.clone()));
                    let _ = tray.set_tooltip(Some(format!(
                        "Zebra Browser Print Relay — ouvindo em 0.0.0.0:{port}, repassando pra 127.0.0.1:{BROWSER_PRINT_PORT}"
                    )));
                    status_item.set_text(format!("Rodando — porta {port} -> {BROWSER_PRINT_PORT}"));
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
