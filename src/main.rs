#![cfg_attr(windows, windows_subsystem = "windows")]

mod icon;
mod proxy;

use std::net::SocketAddr;
use std::thread;

use tao::event::Event;
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::TrayIconBuilder;

const LISTEN_PORT: u16 = 9100;

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
        let target: SocketAddr = ([127, 0, 0, 1], LISTEN_PORT).into();
        let rt = tokio::runtime::Runtime::new().expect("falha ao criar runtime tokio");
        rt.block_on(proxy::run(LISTEN_PORT, target, move |status| {
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
                        "Zebra Browser Print Relay — ouvindo em 0.0.0.0:{port}, repassando pra 127.0.0.1:{port}"
                    )));
                    status_item.set_text(format!("Rodando — porta {port}"));
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
