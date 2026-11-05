use gpui_kit::{Context, EventEmitter};
use oxicord_gateway::{ConnectionState, GatewayEvent};
use oxicord_store::{Changes, Store};

/// The signed in client state. Views observe this entity and listen for the
/// `Changes` it emits to redraw only what moved.
pub struct Session {
    pub store: Store,
    pub connection: ConnectionState,
}

impl EventEmitter<Changes> for Session {}

impl Session {
    pub fn new() -> Self {
        Self {
            store: Store::new(),
            connection: ConnectionState::Connecting,
        }
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
}
