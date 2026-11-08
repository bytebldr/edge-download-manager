//! Backward-compatibility facade for WebSocket & HTTP Native Messaging Server.
//! All logic has been cleanly decomposed into modular sub-crates:
//! - `crate::ws::types`: Inbound/outbound DTOs and commands
//! - `crate::ws::manager`: WsManager state & snapshot coordinator
//! - `crate::ws::http`: HTTP token & Firefox add-on endpoint
//! - `crate::ws::dispatch`: WebSocket framing & dispatch router
//! - `crate::ws::server`: TCP listeners & port fallback
//! - `crate::os::files`: Shell opening & recycle bin handling

pub use crate::os::files::{secure_open_file, secure_open_folder, send_to_recycle_bin};
pub use crate::ws::*;
