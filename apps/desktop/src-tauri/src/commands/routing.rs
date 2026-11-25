use edm_core::routing::{
    default_category_policies, merge_category_policies, validate_proxy_url, CategoryPolicy,
    DomainProxyRule, RequestOverride,
};
use serde::{Deserialize, Serialize};
use tauri::{command, State};
use uuid::Uuid;

use crate::state::AppState;

const DOMAIN_PROXY_RULES_KEY: &str = "routing_domain_proxy_rules";
const CATEGORY_POLICIES_KEY: &str = "routing_category_policies";
const TASK_OVERRIDE_PREFIX: &str = "task_request_override_";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoutingPoliciesPayload {
    pub domain_proxy_rules: Vec<DomainProxyRule>,
    pub category_policies: Vec<CategoryPolicy>,
}

fn load_json_setting<T: serde::de::DeserializeOwned>(
    state: &AppState,
    key: &str,
) -> Result<Option<T>, String> {
    match state.store.get_setting(key) {
        Ok(Some(raw)) if !raw.trim().is_empty() => {
            serde_json::from_str::<T>(&raw).map(Some).map_err(|e| e.to_string())
        }
        Ok(_) => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

fn save_json_setting<T: Serialize + ?Sized>(
    state: &AppState,
    key: &str,
    value: &T,
) -> Result<(), String> {
    let json = serde_json::to_string(value).map_err(|e| e.to_string())?;
    state.store.set_setting(key, &json).map_err(|e| e.to_string())
}

#[command]
pub fn get_routing_policies(state: State<'_, AppState>) -> Result<RoutingPoliciesPayload, String> {
    let domain_proxy_rules =
        load_json_setting::<Vec<DomainProxyRule>>(&state, DOMAIN_PROXY_RULES_KEY)?
            .unwrap_or_default();
    let category_policies =
        load_json_setting::<Vec<CategoryPolicy>>(&state, CATEGORY_POLICIES_KEY)?
            .map(merge_category_policies)
            .unwrap_or_else(default_category_policies);

    Ok(RoutingPoliciesPayload {
        domain_proxy_rules,
        category_policies,
    })
}

#[command]
pub fn save_domain_proxy_rules(
    state: State<'_, AppState>,
    rules: Vec<DomainProxyRule>,
) -> Result<Vec<DomainProxyRule>, String> {
    let mut normalized = Vec::with_capacity(rules.len());
    for mut rule in rules {
        rule.id = rule.id.trim().to_string();
        rule.pattern = rule.pattern.trim().to_string();
        rule.proxy_url = rule.proxy_url.trim().to_string();

        if rule.id.is_empty() {
            return Err("Domain proxy rule id cannot be empty".into());
        }
        if rule.pattern.is_empty() {
            return Err("Domain proxy rule pattern cannot be empty".into());
        }
        validate_proxy_url(&rule.proxy_url)?;
        normalized.push(rule);
    }

    save_json_setting(&state, DOMAIN_PROXY_RULES_KEY, &normalized)?;
    Ok(normalized)
}

#[command]
pub fn save_category_policies(
    state: State<'_, AppState>,
    policies: Vec<CategoryPolicy>,
) -> Result<Vec<CategoryPolicy>, String> {
    let mut normalized = Vec::with_capacity(policies.len());
    for mut policy in policies {
        policy.category = policy.category.trim().to_string();
        if policy.category.is_empty() {
            return Err("Category name cannot be empty".into());
        }
        policy.max_concurrent = policy.max_concurrent.map(|c| c.max(1));
        policy.max_speed_bps = policy.max_speed_bps.filter(|&s| s > 0);
        policy.save_dir = policy
            .save_dir
            .map(|d| d.trim().to_string())
            .filter(|d| !d.is_empty());
        normalized.push(policy);
    }

    save_json_setting(&state, CATEGORY_POLICIES_KEY, &normalized)?;
    Ok(normalized)
}

#[command]
pub fn update_task_request_override(
    state: State<'_, AppState>,
    task_id: String,
    request_override: RequestOverride,
) -> Result<serde_json::Value, String> {
    let trimmed_id = task_id.trim().to_string();
    if trimmed_id.is_empty() {
        return Err("Task ID cannot be empty".into());
    }
    request_override.validate()?;

    let uuid = Uuid::parse_str(&trimmed_id).map_err(|_| format!("Invalid task ID: {trimmed_id}"))?;
    let exists = state
        .store
        .list_all_downloads()
        .map_err(|e| e.to_string())?
        .into_iter()
        .any(|d| d.id == uuid);
    if !exists {
        return Err(format!("Task {trimmed_id} not found"));
    }

    if let Some(ref new_url) = request_override.url {
        state
            .store
            .update_download_url(uuid, new_url.trim())
            .map_err(|e| e.to_string())?;
    }

    let override_key = format!("{TASK_OVERRIDE_PREFIX}{trimmed_id}");
    save_json_setting(&state, &override_key, &request_override)?;

    let is_active = state
        .tasks
        .lock()
        .map(|guard| guard.contains_key(&trimmed_id))
        .unwrap_or(false);

    Ok(serde_json::json!({
        "status": "updated",
        "taskId": trimmed_id,
        "isActive": is_active
    }))
}
