use std::{
    sync::mpsc,
    time::{Duration, Instant},
};

use tao::{
    dpi::LogicalSize,
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoopBuilder},
    platform::{run_return::EventLoopExtRunReturn, windows::EventLoopBuilderExtWindows},
    window::WindowBuilder,
};
use wry::{NewWindowResponse, PermissionResponse, WebViewBuilder};

use crate::{Outcome, handoff};

/// How often the loop wakes to check for a token or the deadline.
const POLL: Duration = Duration::from_millis(250);

pub(crate) fn run() -> Outcome {
    match open() {
        Ok(outcome) => outcome,
        Err(message) => Outcome::Failed(message),
    }
}

fn open() -> Result<Outcome, String> {
    let capability = new_capability()?;
    let (token_tx, token_rx) = mpsc::channel::<String>();

    // `with_any_thread` because gpui already owns the main thread's loop.
    let mut event_loop = EventLoopBuilder::new().with_any_thread(true).build();

    let window = WindowBuilder::new()
        .with_title("Sign in to Discord")
        .with_inner_size(LogicalSize::new(520.0, 780.0))
        .build(&event_loop)
        .map_err(|e| format!("could not open the sign-in window: {e}"))?;

    let expected = capability.clone();
    let webview = WebViewBuilder::new()
        .with_url(handoff::LOGIN_URL)
        // A private session, so nothing from this login outlives the window.
        .with_incognito(true)
        .with_initialization_script(handoff::script(&capability))
        .with_navigation_handler(|url| handoff::is_discord_page(&url))
        .with_new_window_req_handler(|_, _| NewWindowResponse::Deny)
        .with_permission_handler(|_| PermissionResponse::Deny)
        .with_ipc_handler(move |request| {
            // The script runs in every page we allow, but only trust the
            // message if it came from Discord and carries our capability.
            if !handoff::is_discord_page(&request.uri().to_string()) {
                return;
            }
            if let Some(token) = handoff::read_message(request.body(), &expected) {
                let _ = token_tx.send(token.to_owned());
            }
        })
        .build(&window)
        .map_err(|e| {
            format!("could not start the embedded browser (is the WebView2 runtime installed?): {e}")
        })?;

    let deadline = Instant::now() + Duration::from_secs(handoff::TIMEOUT_SECS);
    let mut outcome = Outcome::Closed;

    event_loop.run_return(|event, _, control_flow| {
        *control_flow = ControlFlow::WaitUntil(Instant::now() + POLL);

        if let Ok(token) = token_rx.try_recv() {
            outcome = Outcome::Token(token);
            *control_flow = ControlFlow::Exit;
        } else if Instant::now() >= deadline {
            outcome = Outcome::TimedOut;
            *control_flow = ControlFlow::Exit;
        } else if let Event::WindowEvent {
            event: WindowEvent::CloseRequested,
            ..
        } = event
        {
            *control_flow = ControlFlow::Exit;
        }
    });

    // The webview must go before its window.
    drop(webview);
    drop(window);
    Ok(outcome)
}

/// A secret the page has to repeat back, so a stray script in the page
/// can't pose as our own.
fn new_capability() -> Result<String, String> {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).map_err(|e| format!("no secure randomness available: {e}"))?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}
