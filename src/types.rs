use alloy::{
    hex,
    primitives::keccak256,
    signers::{local::PrivateKeySigner, Signer},
};
use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{fmt, str::FromStr};
use strum_macros::{Display, EnumString};

#[derive(Debug, Clone, EnumString, Display, Deserialize, Serialize)]
#[strum(serialize_all = "kebab-case")]
#[serde(rename_all = "kebab-case")]
pub enum ModelKind {
    AdaptiveThreshold,
    DistributionAnalysis,
    MovingAverage,
    Percentile,
    TimeSeries,
    LastMin,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum AgentKind {
    /// Will publish the standard estimate from the node
    Node,
    /// Will publish the actual min price for new blocks
    Target,
    /// Will publish a estimate based on the model kind
    Model(ModelKind),
}

impl fmt::Display for AgentKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AgentKind::Node => write!(f, "node"),
            AgentKind::Target => write!(f, "target"),
            AgentKind::Model(kind) => write!(f, "{}", kind),
        }
    }
}

impl FromStr for AgentKind {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // First try to parse as ModelKind
        if let Ok(model_kind) = ModelKind::from_str(s) {
            return Ok(AgentKind::Model(model_kind));
        }

        // Then match on known string values
        match s.to_lowercase().as_str() {
            "node" => Ok(AgentKind::Node),
            "target" => Ok(AgentKind::Target),
            _ => Err(format!("Unknown mode: {}", s)),
        }
    }
}

#[derive(Debug, Clone, EnumString, Display, Deserialize, Serialize)]
#[strum(serialize_all = "UPPERCASE")]
#[serde(rename_all = "UPPERCASE")]
pub enum FeeUnit {
    Gwei,
}

#[derive(Debug, Clone, EnumString, Display, Deserialize, Serialize)]
#[strum(serialize_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum AgentPayloadKind {
    Estimate,
    Target,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AgentPayload {
    /// The block height this payload is valid from
    pub from_block: u64,
    /// How fast the settlement time is for this payload
    pub settlement: Settlement,
    /// The exact time the prediction is captured UTC.
    pub timestamp: DateTime<Utc>,
    /// The unit the fee is denominated in (e.g. gwei, sats)
    pub unit: FeeUnit,
    /// The name of the chain the estimations are for (eg. ethereum, bitcoin, base)
    pub system: String,
    /// mainnet, etc.
    pub network: String,
    /// The estimated price
    pub price: f64,
    pub kind: AgentPayloadKind,
}

impl AgentPayload {
    /// Hashes (keccak256) the payload and returns as bytes
    pub fn hash(&self) -> Vec<u8> {
        let json = json!({
            "timestamp": self.timestamp,
            "system": self.system,
            "network": self.network,
            "settlement": self.settlement,
            "from_block": self.from_block,
            "price": self.price,
            "unit": self.unit,
            "kind": self.kind,
        });

        let bytes = json.to_string().as_bytes().to_vec();
        let message_hash = keccak256(&bytes).to_string();
        message_hash.as_bytes().to_vec()
    }

    pub async fn sign(&self, signer_key: &str) -> Result<String> {
        let message = self.hash();
        let signer: PrivateKeySigner = signer_key.parse()?;
        let signature = signer.sign_message(&message).await?;
        let hex_signature = hex::encode(signature.as_bytes());

        Ok(format!("0x{}", hex_signature))
    }
}

#[derive(Debug, Clone, EnumString, Display, Deserialize, Serialize)]
#[strum(serialize_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum Env {
    Stage,
    Prod,
    Local,
}

#[derive(Debug, Clone, EnumString, Display, Deserialize, Serialize, Hash, PartialEq, Eq)]
#[strum(serialize_all = "lowercase")]
#[serde(rename_all = "lowercase")]
/// Settlement time that is translated to number of blocks based on chain block time
pub enum Settlement {
    /// Next block
    Immediate,
    /// 15 seconds
    Fast,
    /// 15 minutes
    Medium,
    /// 1 hour
    Slow,
}
