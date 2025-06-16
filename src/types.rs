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

use crate::chain::{sign::PayloadSigner, types::SignedOraclePayloadV2};

#[derive(Debug, Clone, EnumString, Display, Deserialize, Serialize)]
#[strum(serialize_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum ModelKind {
    AdaptiveThreshold,
    DistributionAnalysis,
    MovingAverage,
    Percentile,
    TimeSeries,
    LastMin,
    PendingFloor,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
#[serde(from = "String")]
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

impl From<String> for AgentKind {
    fn from(s: String) -> Self {
        AgentKind::from_str(&s).unwrap_or_else(|_| panic!("Invalid AgentKind: {}", s))
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
    pub system: System,
    /// mainnet, etc.
    pub network: Network,
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

    pub fn network_signature(self, signer_key: &str) -> Result<String> {
        let mut opv2 = SignedOraclePayloadV2 {
            payload: self.into(),
            signature: None,
        };

        let mut buf = vec![];
        let signer: PrivateKeySigner = signer_key.parse()?;
        opv2.to_signed_payload(&mut buf, signer)?;

        let hex_signature = hex::encode(opv2.signature.unwrap().as_bytes());

        Ok(format!("0x{}", hex_signature))
    }
}

#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize, Deserialize, Display, EnumString)]
#[strum(serialize_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum System {
    Ethereum,
    Base,
    Polygon,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize, Deserialize, Display, EnumString)]
#[strum(serialize_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum Network {
    Mainnet,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct SystemNetworkKey {
    pub system: System,
    pub network: Network,
}

impl SystemNetworkKey {
    pub fn new(system: System, network: Network) -> Self {
        Self { system, network }
    }

    pub fn to_chain_id(&self) -> u64 {
        match self {
            SystemNetworkKey {
                system: System::Ethereum,
                network: Network::Mainnet,
            } => 1,
            SystemNetworkKey {
                system: System::Base,
                network: Network::Mainnet,
            } => 8453,
            SystemNetworkKey {
                system: System::Polygon,
                network: Network::Mainnet,
            } => 137,
        }
    }

    pub fn to_block_time(&self) -> u64 {
        match self {
            SystemNetworkKey {
                system: System::Ethereum,
                network: Network::Mainnet,
            } => 12000,
            SystemNetworkKey {
                system: System::Base,
                network: Network::Mainnet,
            } => 2000,
            SystemNetworkKey {
                system: System::Polygon,
                network: Network::Mainnet,
            } => 2000,
        }
    }
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct SystemNetworkSettlementKey {
    pub system: System,
    pub network: Network,
    pub settlement: Settlement,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct BlockWindow {
    pub start: u64,
    pub end: u64,
}

impl fmt::Display for BlockWindow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}-{}", self.start, self.end)
    }
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

#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize)]
pub struct AgentKey {
    pub settlement: Settlement,
    pub agent_id: String,
    pub system: System,
    pub network: Network,
}

#[derive(Debug, Clone, Serialize)]
pub struct AgentScore {
    pub agent_id: String,
    pub inclusion_cov: Option<f64>,
    pub overpayment_cov: Option<f64>,
    pub total_score: Option<f64>,
    pub inclusion_rate: f64,
    pub avg_overpayment: Option<f64>,
}
