use super::constants::AGENT_PUBLISH_PATH;
use crate::types::AgentPayload;
use anyhow::Result;
use reqwest::Client;
use serde_json::json;

pub async fn publish_agent_payload(
    client: &Client,
    collector_endpoint: &str,
    signer_key: &str,
    payload: &AgentPayload,
) -> Result<()> {
    let signature = payload.sign(signer_key).await?;
    let network_signature = payload.clone().network_signature(signer_key)?;

    let json = json!({
        "payload": payload,
        "signature": signature,
        "network_signature": network_signature,
    });

    tracing::debug!("Publishing agent payload: {:?}", json);

    let response = client
        .post(format!("{}{}", collector_endpoint, AGENT_PUBLISH_PATH))
        .json(&json)
        .send()
        .await?;

    let status = response.status();
    let body = response.text().await?;

    if !status.is_success() {
        return Err(anyhow::anyhow!("Failed to publish agent payload: {}", body));
    }

    Ok(())
}
