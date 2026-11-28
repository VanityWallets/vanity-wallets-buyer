#![windows_subsystem = "windows"]

mod dialogs;
mod logic;
mod patterns;
mod split_key;
mod vault;

use logic::{BuyerApp, Effect};
use serde_json::Value;
use std::borrow::Cow;
use tao::dpi::LogicalSize;
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy};
use tao::window::{Icon, ResizeDirection, Window, WindowBuilder};
use wry::http::{header, Request, Response, StatusCode};
use wry::{WebContext, WebView, WebViewBuilder};
use zeroize::Zeroize;

const ICON_RGBA: &[u8] = include_bytes!("../assets/icon_256.rgba");
const START_URL: &str = "http://app.localhost/index.html";
const ORIGIN: &str = "http://app.localhost/";

fn ui_file(path: &str) -> Option<(&'static [u8], &'static str)> {
    Some(match path {
        "" | "index.html" => (include_bytes!("../ui/index.html"), "text/html; charset=utf-8"),
        "app.css" => (include_bytes!("../ui/app.css"), "text/css; charset=utf-8"),
        "app.js" => (include_bytes!("../ui/app.js"), "text/javascript; charset=utf-8"),
        "logo.png" => (include_bytes!("../ui/logo.png"), "image/png"),
        "fonts/plex-400.ttf" => (include_bytes!("../ui/fonts/plex-400.ttf"), "font/ttf"),
        "fonts/plex-500.ttf" => (include_bytes!("../ui/fonts/plex-500.ttf"), "font/ttf"),
        "fonts/plex-600.ttf" => (include_bytes!("../ui/fonts/plex-600.ttf"), "font/ttf"),
        "fonts/mono-400.ttf" => (include_bytes!("../ui/fonts/mono-400.ttf"), "font/ttf"),
        "fonts/mono-500.ttf" => (include_bytes!("../ui/fonts/mono-500.ttf"), "font/ttf"),
        _ => return net_icon(path),
    })
}

fn net_icon(path: &str) -> Option<(&'static [u8], &'static str)> {
    macro_rules! icons {
        ($($name:literal),*) => {
            match path.strip_prefix("net/")?.strip_suffix(".svg")? {
                $($name => include_bytes!(concat!("../ui/net/", $name, ".svg")).as_slice(),)*
                _ => return None,
            }
        };
    }
    Some((
        icons!("akt", "arb", "atom", "avax", "base", "bnb", "btc", "dash", "doge", "dydx", "eth", "hype", "juno", "linea", "ltc",
            "noble", "op", "osmo", "pol", "stars", "tia", "tron", "xrp", "zksync"),
        "image/svg+xml",
    ))
}

const CSP: &str = "default-src 'none'; script-src 'self'; style-src 'self'; font-src 'self'; img-src 'self'; \
                   connect-src 'none'; frame-src 'none'; object-src 'none'; base-uri 'none'; form-action 'none'";

fn serve(request: Request<Vec<u8>>) -> Response<Cow<'static, [u8]>> {
    let path = request.uri().path().trim_start_matches('/');
    match ui_file(path) {
        Some((body, mime)) => Response::builder()
            .header(header::CONTENT_TYPE, mime)
            .header("Content-Security-Policy", CSP)
            .header("X-Content-Type-Options", "nosniff")
            .header(header::CACHE_CONTROL, "no-store")
            .body(Cow::Borrowed(body))
            .unwrap_or_else(|_| Response::new(Cow::Borrowed(&[]))),
        None => Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Cow::Borrowed(&b""[..]))
            .unwrap_or_else(|_| Response::new(Cow::Borrowed(&[]))),
    }
}

#[cfg(windows)]
fn no_crash_reports() {
    use windows::Win32::System::Diagnostics::Debug::{
        SetErrorMode, SEM_FAILCRITICALERRORS, SEM_NOGPFAULTERRORBOX,
    };
    unsafe {
        SetErrorMode(SEM_FAILCRITICALERRORS | SEM_NOGPFAULTERRORBOX);
    }
    std::panic::set_hook(Box::new(|_| std::process::abort()));
}

fn fatal(text: &str) {
    #[cfg(windows)]
    {
        use windows::core::HSTRING;
        use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK};
        unsafe {
            MessageBoxW(None, &HSTRING::from(text), &HSTRING::from("Vanity Wallets Buyer"), MB_OK | MB_ICONERROR);
        }
    }
    #[cfg(not(windows))]
    eprintln!("{text}");
}

fn copy_secret(text: &str) -> bool {
    #[cfg(windows)]
    use arboard::SetExtWindows;
    let Ok(mut clipboard) = arboard::Clipboard::new() else {
        return false;
    };
    let set = clipboard.set();
    #[cfg(windows)]
    let set = set.exclude_from_cloud().exclude_from_history().exclude_from_monitoring();
    set.text(text.to_string()).is_ok()
}

fn copy_text(text: &str) -> bool {
    arboard::Clipboard::new().and_then(|mut c| c.set_text(text.to_string())).is_ok()
}

fn clear_secret(text: &str) {
    if let Ok(mut clipboard) = arboard::Clipboard::new() {
        if clipboard.get_text().ok().as_deref() == Some(text) {
            let _ = clipboard.clear();
        }
    }
}

enum UserEvent {
    Ipc(String),
    Picked(Option<std::path::PathBuf>),
}

fn webview_data_dir() -> std::path::PathBuf {
    match std::env::var_os("LOCALAPPDATA") {
        Some(base) => std::path::PathBuf::from(base).join("VanityBuyer").join("WebView2"),
        None => std::env::temp_dir().join("VanityBuyer-WebView2"),
    }
}

fn hwnd_of(window: &Window) -> isize {
    #[cfg(windows)]
    {
        use tao::platform::windows::WindowExtWindows;
        window.hwnd()
    }
    #[cfg(not(windows))]
    {
        let _ = window;
        0
    }
}

fn push_state(webview: &WebView, app: &BuyerApp, window: &Window) {
    let mut state = app.view();
    state["maximized"] = Value::Bool(window.is_maximized());
    let mut text = state.to_string();
    let mut script = format!("window.vb && window.vb.render({text});");
    let _ = webview.evaluate_script(&script);
    text.zeroize();
    script.zeroize();
}

#[cfg(windows)]
fn already_running() -> bool {
    use windows::core::w;
    use windows::Win32::Foundation::{GetLastError, ERROR_ALREADY_EXISTS};
    use windows::Win32::System::Threading::CreateMutexW;
    use windows::Win32::UI::WindowsAndMessaging::{FindWindowW, IsIconic, SetForegroundWindow, ShowWindow, SW_RESTORE};
    unsafe {
        let mutex = CreateMutexW(None, false, w!("Local\\VanityWalletsBuyer.SingleInstance"));
        if mutex.is_err() || GetLastError() != ERROR_ALREADY_EXISTS {
            return false;
        }
        if let Ok(hwnd) = FindWindowW(None, w!("Vanity Wallets Buyer")) {
            if IsIconic(hwnd).as_bool() {
                let _ = ShowWindow(hwnd, SW_RESTORE);
            }
            let _ = SetForegroundWindow(hwnd);
        }
        true
    }
}

fn main() {
    #[cfg(windows)]
    no_crash_reports();
    #[cfg(windows)]
    if already_running() {
        return;
    }

    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    let proxy: EventLoopProxy<UserEvent> = event_loop.create_proxy();
    let icon = Icon::from_rgba(ICON_RGBA.to_vec(), 256, 256).ok();
    let builder = WindowBuilder::new();
    #[cfg(windows)]
    let builder = {
        use tao::platform::windows::WindowBuilderExtWindows;
        builder.with_undecorated_shadow(false)
    };
    let window = match builder
        .with_title("Vanity Wallets Buyer")
        .with_inner_size(LogicalSize::new(1200.0, 800.0))
        .with_min_inner_size(LogicalSize::new(880.0, 600.0))
        .with_decorations(false)
        .with_transparent(true)
        .with_visible(false)
        .with_window_icon(icon)
        .build(&event_loop)
    {
        Ok(w) => w,
        Err(e) => {
            fatal(&format!("Не удалось открыть окно / Could not open the window: {e}"));
            return;
        }
    };

    let mut context = WebContext::new(Some(webview_data_dir()));
    let ipc_proxy = proxy.clone();
    let builder = WebViewBuilder::new_with_web_context(&mut context)
        .with_url(START_URL)
        .with_transparent(true)
        .with_background_color((0, 0, 0, 0))
        .with_devtools(false)
        .with_hotkeys_zoom(false)
        .with_custom_protocol("app".into(), |_id, request| serve(request))
        .with_ipc_handler(move |request: Request<String>| {
            let _ = ipc_proxy.send_event(UserEvent::Ipc(request.into_body()));
        })
        .with_navigation_handler(|url| url.starts_with(ORIGIN))
        .with_new_window_req_handler(|_url, _features| wry::NewWindowResponse::Deny);
    #[cfg(windows)]
    let builder = {
        use wry::WebViewBuilderExtWindows;
        builder.with_additional_browser_args(
            "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection,AutofillServerCommunication \
             --disable-background-networking --disable-component-update --disable-domain-reliability \
             --disable-sync --no-pings --disable-breakpad --disable-crash-reporter --no-first-run",
        )
    };
    let webview = match builder.build(&window) {
        Ok(w) => w,
        Err(e) => {
            fatal(&format!(
                "Не удалось запустить встроенный движок Windows WebView2. Установите «Microsoft Edge WebView2 Runtime» \
                 с сайта Microsoft и запустите программу снова.\n\nCould not start Windows WebView2. Install the \
                 “Microsoft Edge WebView2 Runtime” from Microsoft and start the program again.\n\n{e}"
            ));
            return;
        }
    };

    let mut app = BuyerApp::new();
    let mut shown = false;
    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        match event {
            Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => {
                if app.unsaved_keys() {
                    app.ask_close();
                    push_state(&webview, &app, &window);
                } else {
                    *control_flow = ControlFlow::Exit;
                }
            }
            Event::WindowEvent { event: WindowEvent::Resized(_), .. } if shown => {
                let max = window.is_maximized();
                let _ = webview.evaluate_script(&format!("window.vb && window.vb.maximized({max});"));
            }
            Event::UserEvent(UserEvent::Picked(path)) => {
                app.pick_done(path);
                push_state(&webview, &app, &window);
            }
            Event::UserEvent(UserEvent::Ipc(text)) => {
                let Ok(msg) = serde_json::from_str::<Value>(&text) else { return };
                match msg.get("a").and_then(Value::as_str).unwrap_or("") {
                    "ready" => {
                        push_state(&webview, &app, &window);
                        if !shown {
                            window.set_visible(true);
                            shown = true;
                        }
                    }
                    "drag" => {
                        let _ = window.drag_window();
                    }
                    "resize" => {
                        let dir = match msg.get("dir").and_then(Value::as_str).unwrap_or("") {
                            "n" => ResizeDirection::North,
                            "s" => ResizeDirection::South,
                            "e" => ResizeDirection::East,
                            "w" => ResizeDirection::West,
                            "ne" => ResizeDirection::NorthEast,
                            "nw" => ResizeDirection::NorthWest,
                            "se" => ResizeDirection::SouthEast,
                            _ => ResizeDirection::SouthWest,
                        };
                        if !window.is_maximized() {
                            let _ = window.drag_resize_window(dir);
                        }
                    }
                    "paste" => {
                        let target = if msg.get("target").and_then(Value::as_str) == Some("key") { "key" } else { "proof" };
                        let mut text: String = arboard::Clipboard::new()
                            .and_then(|mut c| c.get_text())
                            .unwrap_or_default()
                            .chars()
                            .take(200_000)
                            .collect();
                        let mut json = serde_json::to_string(&text).unwrap_or_else(|_| "\"\"".into());
                        let mut script = format!("window.vb && window.vb.pasted(\"{target}\", {json});");
                        let _ = webview.evaluate_script(&script);
                        text.zeroize();
                        json.zeroize();
                        script.zeroize();
                    }
                    "maximize" => window.set_maximized(!window.is_maximized()),
                    "minimize" => window.set_minimized(true),
                    "close" if app.unsaved_keys() => {
                        app.ask_close();
                        push_state(&webview, &app, &window);
                    }
                    "close" | "close_confirm" => *control_flow = ControlFlow::Exit,
                    _ => {
                        match app.act(&msg) {
                            Effect::None => {}
                            Effect::CopyText(t) => {
                                copy_text(&t);
                            }
                            Effect::CopySecret(mut s) => {
                                let ok = copy_secret(&s);
                                app.key_copied(s.clone(), ok);
                                s.zeroize();
                            }
                            Effect::Dialog(d) => {
                                let proxy = proxy.clone();
                                dialogs::FileDialog {
                                    save: d.save,
                                    title: d.title,
                                    file_name: d.file_name,
                                    start_dir: d.start_dir,
                                    filter: d.filter,
                                    owner: hwnd_of(&window),
                                }
                                .spawn(move |path| {
                                    let _ = proxy.send_event(UserEvent::Picked(path));
                                });
                            }
                        }
                        push_state(&webview, &app, &window);
                    }
                }
            }
            Event::LoopDestroyed => {
                if let Some(mut key) = app.copied.take() {
                    clear_secret(&key);
                    key.zeroize();
                }
                app.forget_secrets();
            }
            _ => {}
        }
    });
}
