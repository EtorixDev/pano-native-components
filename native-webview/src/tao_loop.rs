use std::{path::PathBuf, sync::OnceLock};

use tao::{
    event::{Event, StartCause, WindowEvent},
    event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy},
    window::{Window, WindowBuilder},
};
use wry::{WebContext, WebView, WebViewBuilder, WebViewBuilderExtWindows, dpi::LogicalSize};

use crate::webview_event::{WebViewIncomingEvent, WebViewOutgoingEvent};

static WEBVIEW_INCOMING_EVENT_SENDER: OnceLock<EventLoopProxy<WebViewIncomingEvent>> =
    OnceLock::new();

static WEBVIEW_INITIAL_EVENT: OnceLock<WebViewIncomingEvent> = OnceLock::new();

pub fn send_incoming_webview_event(user_event: WebViewIncomingEvent) {
    if let Some(proxy) = WEBVIEW_INCOMING_EVENT_SENDER.get() {
        match proxy.send_event(user_event) {
            Ok(_) => {}
            Err(e) => eprintln!("Error sending message to event loop: {e}"),
        }
    } else {
        let _ = WEBVIEW_INITIAL_EVENT.set(user_event);
        eprintln!("Event loop not running");
    }
}

pub fn get_cookies_for_url(webview: &WebView, url: &str) -> Vec<String> {
    webview
        .cookies_for_url(url)
        .unwrap_or_default()
        .iter()
        .map(|cookie| format!("{}={}", cookie.name(), cookie.value(),))
        .collect::<Vec<_>>()
}

pub fn event_loop(jni_callback: impl Fn(WebViewOutgoingEvent) + 'static) {
    use tao::platform::windows::EventLoopBuilderExtWindows;

    let event_loop = EventLoopBuilder::<WebViewIncomingEvent>::with_user_event()
        .with_any_thread(true)
        .build();

    let proxy = event_loop.create_proxy();
    WEBVIEW_INCOMING_EVENT_SENDER.set(proxy).unwrap();

    let mut webview_window: Option<(Window, WebView, WebContext)> = None;

    event_loop.run(move |event, target, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::NewEvents(cause) => {
                if StartCause::Init == cause
                    && let Some(event) = WEBVIEW_INITIAL_EVENT.get()
                {
                    send_incoming_webview_event(event.clone());
                }
            }

            Event::UserEvent(WebViewIncomingEvent::LaunchWebView(
                url,
                callback_prefix,
                cookies_url,
                data_dir,
                proxy_type,
                proxy_host,
                proxy_port,
            )) => {
                let window = WindowBuilder::new()
                    .with_title("WebView")
                    .with_inner_size(LogicalSize::new(640.0, 480.0))
                    .with_focused(true)
                    .build(target)
                    .unwrap();

                let mut context = WebContext::new(Some(PathBuf::from(data_dir)));

                let mut builder = WebViewBuilder::new_with_web_context(&mut context)
                    // WebViewBuilder::new()
                    .with_url(&url)
                    .with_navigation_handler(move |url| {
                        let is_callback = url.starts_with(&callback_prefix);

                        if is_callback {
                            send_incoming_webview_event(WebViewIncomingEvent::SendCallback(
                                url,
                                cookies_url.clone(),
                            ));
                            false
                        } else {
                            true
                        }
                    });

                if !proxy_type.is_empty() && !proxy_host.is_empty() && proxy_port != 0 {
                    builder = builder.with_additional_browser_args(format!(
                        "--proxy-server={}://{}:{}",
                        proxy_type, proxy_host, proxy_port
                    ))
                }

                let webview = builder.build(&window);

                match webview {
                    Ok(webview) => {
                        webview_window = Some((window, webview, context));
                    }
                    Err(e) => {
                        webview_window = None;
                        eprintln!("Failed to create WebView: {e}");
                    }
                }
            }

            Event::UserEvent(WebViewIncomingEvent::SendCallback(url, cookies_url)) => {
                if let Some((_, webview, _)) = &webview_window {
                    let cookies = get_cookies_for_url(webview, &cookies_url);
                    let new_event = WebViewOutgoingEvent::WebViewCallback(url, cookies);
                    jni_callback(new_event);
                }
            }

            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                webview_window.take();
            }

            Event::UserEvent(WebViewIncomingEvent::Close) => {
                if let Some((_, webview, _)) = &webview_window {
                    let _ = webview.clear_all_browsing_data();
                }
                webview_window.take();
            }

            Event::UserEvent(WebViewIncomingEvent::Quit) => {
                if let Some((_, webview, _)) = &webview_window {
                    let _ = webview.clear_all_browsing_data();
                }
                webview_window.take();
                *control_flow = ControlFlow::Exit;
            }

            _ => (),
        }
    });
}
