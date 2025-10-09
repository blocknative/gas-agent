use crate::chain::{sign::PayloadSigner, types::SignedOraclePayloadV2};
#[cfg(test)]
use alloy::signers::Signature;
use alloy::{
    hex,
    primitives::{keccak256, B256},
    signers::{local::PrivateKeySigner, Signer},
};
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::{fmt, str::FromStr};
use strum_macros::{Display, EnumString};

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
            AgentKind::Model(kind) => write!(f, "{kind}"),
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
            _ => Err(format!("Unknown mode: {s}")),
        }
    }
}

impl From<String> for AgentKind {
    fn from(s: String) -> Self {
        AgentKind::from_str(&s).unwrap_or_else(|_| panic!("Invalid AgentKind: {s}"))
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AgentPayload {
    /// Schema version string for signed payloads
    #[serde(default = "AgentPayload::schema_version")]
    pub schema_version: String,
    /// The block height this payload is valid from
    pub from_block: u64,
    /// How fast the settlement time is for this payload
    pub settlement: Settlement,
    /// The exact time the estimate is captured UTC.
    pub timestamp: DateTime<Utc>,
    /// The name of the chain the estimations are for (eg. ethereum, base)
    pub system: System,
    /// mainnet, etc.
    pub network: Network,
    /// The unit of the `price` field (currently only wei)
    #[serde(default = "PriceUnit::default_wei")]
    pub unit: PriceUnit,
    /// The estimated price as a decimal string. Interpretation depends on `unit`.
    /// For `wei`, this MUST be an integer decimal string with no leading zeros (except "0").
    pub price: String,
}

impl AgentPayload {
    fn schema_version() -> String {
        "1".to_string()
    }

    // --- Canonical JSON signing helpers ---
    fn timestamp_ns_string(&self) -> String {
        let secs = self.timestamp.timestamp() as i128;
        let nanos = self.timestamp.timestamp_subsec_nanos() as i128;
        let ts_ns: i128 = secs * 1_000_000_000 + nanos;
        assert!(ts_ns >= 0, "negative timestamps are not supported");
        ts_ns.to_string()
    }

    /// Build minified canonical JSON with lexicographically sorted keys including exactly the AgentPayload fields.
    pub fn canonical_json_string(&self) -> String {
        let schema_version = self.schema_version.clone();
        let from_block = self.from_block.to_string();
        let settlement = self.settlement.to_string().to_lowercase();
        let timestamp = self.timestamp_ns_string();
        let system = self.system.to_string().to_lowercase();
        let network = self.network.to_string().to_lowercase();
        let price = self.price.clone();
        let unit = self.unit.to_string().to_lowercase();

        format!(
            "{{\"from_block\":\"{}\",\"network\":\"{}\",\"price\":\"{}\",\"schema_version\":\"{}\",\"settlement\":\"{}\",\"system\":\"{}\",\"timestamp\":\"{}\",\"unit\":\"{}\"}}",
            from_block, network, price, schema_version, settlement, system, timestamp, unit
        )
    }

    fn canonical_digest(&self) -> B256 {
        let json = self.canonical_json_string();
        keccak256(json.as_bytes())
    }

    /// Sign the Keccak-256 digest of the canonical JSON per spec v1.0.0.
    pub async fn sign(&self, signer_key: &str) -> Result<String> {
        let signer: PrivateKeySigner = signer_key.parse()?;
        let digest = self.canonical_digest();
        let signature = signer.sign_hash(&digest).await?;
        let hex_signature = hex::encode(signature.as_bytes());
        Ok(format!("0x{hex_signature}"))
    }

    pub fn network_signature(self, signer_key: &str) -> Result<String> {
        let mut opv2 = SignedOraclePayloadV2 {
            payload: self.into(),
            signature: None,
        };

        let mut buf = vec![];
        let signer: PrivateKeySigner = signer_key.parse()?;
        opv2.to_signed_payload(&mut buf, signer)?;

        let hex_signature = hex::encode(
            opv2.signature
                .as_ref()
                .context("signature should be set after signing")?
                .as_bytes(),
        );

        Ok(format!("0x{hex_signature}"))
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

#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize, Deserialize, Display, EnumString)]
#[strum(serialize_all = "lowercase")]
#[serde(rename_all = "lowercase")]
/// Unit for the price field in AgentPayload
pub enum PriceUnit {
    Wei,
}

impl PriceUnit {
    pub fn default_wei() -> Self {
        PriceUnit::Wei
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::{primitives::Address, signers::local::PrivateKeySigner};
    use chrono::{DateTime, Utc};

    // Tests-only helper to validate a canonical JSON signature against this payload
    impl AgentPayload {
        pub fn validate_signature(&self, signature: &str) -> anyhow::Result<Address> {
            let sig_bytes = signature.strip_prefix("0x").unwrap_or(signature);
            let sig_bytes = alloy::hex::decode(sig_bytes)?;
            let signature = Signature::from_raw(&sig_bytes)?;
            let digest = self.canonical_digest();
            let recovered = signature.recover_address_from_prehash(&digest)?;
            Ok(recovered)
        }
    }

    #[tokio::test]
    async fn test_canonical_sign_and_recover_roundtrip() {
        let timestamp = DateTime::parse_from_rfc3339("2024-01-01T12:00:00.500000000Z")
            .unwrap()
            .with_timezone(&Utc);
        // Fixed private key for reproducibility (DO NOT USE IN PROD)
        let sk_hex = "0x59c6995e998f97a5a0044976f3ac3b8c9f27a7d9b3bcd2b0d7aeb5f3e9eae7c6";
        let signer: PrivateKeySigner = sk_hex.parse().unwrap();
        let payload = AgentPayload {
            schema_version: "1".to_string(),
            from_block: 12345,
            settlement: Settlement::Fast,
            timestamp,
            system: System::Ethereum,
            network: Network::Mainnet,
            unit: PriceUnit::Wei,
            price: "20000000000".to_string(),
        };

        // Sign
        let sig = payload.sign(sk_hex).await.unwrap();
        assert!(sig.starts_with("0x"));

        // Recover and ensure matches the signer address
        let recovered = payload.validate_signature(&sig).unwrap();
        assert_eq!(recovered, signer.address());
    }
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
