use std::collections::HashMap;

use gpui_kit::{Context, EventEmitter};
use oxicord_gateway::{ClientStatus, ConnectionState, GatewayEvent};
use oxicord_model::{Message, Snowflake};
use oxicord_store::{Changes, Store};

/// The signed in client state. Views observe this entity and listen for the
/// `Changes` it emits to redraw only what moved.
pub struct Session {
    pub store: Store,
    pub connection: ConnectionState,
    /// Channels whose last history request failed, with the reason to show.
    pub history_errors: HashMap<Snowflake, String>,
    /// The latest thing that went wrong, shown until dismissed.
    pub notice: Option<String>,
    /// Whether the member list is showing beside the chat.
    pub show_members: bool,
    /// The status we last chose, which the server won't echo back to us.
    pub status: ClientStatus,
}

impl EventEmitter<Changes> for Session {}

impl Session {
    pub fn new() -> Self {
        Self {
            store: Store::new(),
            connection: ConnectionState::Connecting,
            history_errors: HashMap::new(),
            notice: None,
            show_members: true,
            status: ClientStatus::default(),
        }
    }

    pub fn set_notice(&mut self, notice: Option<String>, cx: &mut Context<Self>) {
        self.notice = notice;
        cx.notify();
    }

    pub fn handle(&mut self, event: GatewayEvent, cx: &mut Context<Self>) {
        match event {
            GatewayEvent::Dispatch(event) => {
                let changes = self.store.apply(event);
                if !changes.is_empty() {
                    cx.emit(changes);
                    cx.notify();
                }
            }
            GatewayEvent::State(state) => {
                self.connection = state;
                cx.notify();
            }
        }
    }

    /// Folds in a page of history. A failure is remembered and announced the
    /// same way, so a view waiting on the channel stops waiting.
    pub fn apply_history(
        &mut self,
        channel: Snowflake,
        result: Result<Vec<Message>, String>,
        requested: usize,
        cx: &mut Context<Self>,
    ) {
        let changes = match result {
            Ok(page) => {
                self.history_errors.remove(&channel);
                self.store.set_history(channel, page, requested)
            }
            Err(error) => {
                self.history_errors.insert(channel, error);
                let mut changes = Changes::default();
                changes.messages.insert(channel);
                changes
            }
        };
        cx.emit(changes);
        cx.notify();
    }
}
