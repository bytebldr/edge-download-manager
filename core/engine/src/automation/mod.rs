pub mod rpc;
pub mod webhooks;

pub use rpc::{parse_aria2_rpc, RpcAction, RpcError, RpcRequest, RpcResponse};
pub use webhooks::{dispatch_completion_webhook, validate_webhook_url, AutomationConfig};
