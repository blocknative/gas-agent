use crate::chain::{sign::PayloadSigner, types::SignedOraclePayloadV2};
use alloy::{
    hex,
    primitives::{keccak256, Address, B256, U256},
    signers::{local::PrivateKeySigner, SignerSync},
};
#[cfg(test)]
use alloy::signers::Signature;
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
    /// Schema/version for signed payloads (EIP-712 domain version)
    #[serde(default = "AgentPayload::schema_version")]
    pub schema_version: String,
    /// The block height this payload is valid from
    pub from_block: u64,
    /// How fast the settlement time is for this payload
    pub settlement: Settlement,
    /// The exact time the estimate is captured UTC.
    pub timestamp: DateTime<Utc>,
    /// The name of the chain the estimations are for (eg. ethereum, bitcoin, base)
    pub system: System,
    /// mainnet, etc.
    pub network: Network,
    /// The estimated price in wei (always denominated in wei)
    pub price: U256,
}

impl AgentPayload {
    fn schema_version() -> String {
        "1".to_string()
    }

    // --- EIP-712 helpers ---
    fn typehash_domain() -> B256 {
        // keccak256("EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)")
        keccak256(
            "EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)"
                .as_bytes(),
        )
    }

    fn typehash_agent_payload() -> B256 {
        // keccak256("AgentPayload(string schema_version,uint256 timestamp,string system,string network,string settlement,uint256 from_block,uint256 price)")
        keccak256(
            "AgentPayload(string schema_version,uint256 timestamp,string system,string network,string settlement,uint256 from_block,uint256 price)"
                .as_bytes(),
        )
    }

    fn keccak_string(val: &str) -> B256 {
        keccak256(val.as_bytes())
    }

    fn encode_u256_bytes_be(x: impl Into<u128>) -> [u8; 32] {
        let v: u128 = x.into();
        let mut out = [0u8; 32];
        out[16..].copy_from_slice(&v.to_be_bytes());
        out
    }

    fn encode_u256_u64(x: u64) -> [u8; 32] {
        Self::encode_u256_bytes_be(x as u128)
    }

    fn encode_address(addr: Address) -> [u8; 32] {
        let mut out = [0u8; 32];
        out[12..].copy_from_slice(addr.as_slice());
        out
    }

    fn domain_separator(&self, chain_id: u64) -> B256 {
        let typehash = Self::typehash_domain();
        let name_hash = Self::keccak_string("Gas Network AgentPayload");
        let version_hash = Self::keccak_string(&self.schema_version);
        let chain_id_enc = Self::encode_u256_u64(chain_id);
        let verifying = Self::encode_address(Address::ZERO);

        let mut enc = Vec::with_capacity(32 * 5);
        enc.extend_from_slice(typehash.as_slice());
        enc.extend_from_slice(name_hash.as_slice());
        enc.extend_from_slice(version_hash.as_slice());
        enc.extend_from_slice(&chain_id_enc);
        enc.extend_from_slice(&verifying);
        keccak256(&enc)
    }

    fn struct_hash(&self) -> B256 {
        let typehash = Self::typehash_agent_payload();

        // timestamp in ns since epoch
        let secs = self.timestamp.timestamp() as i128;
        let nanos = self.timestamp.timestamp_subsec_nanos() as i128;
        let ts_ns: i128 = secs * 1_000_000_000 + nanos;
        let ts_ns_u: u128 = ts_ns as u128;

        // lowercase strings
        let system = self.system.to_string().to_lowercase();
        let network = self.network.to_string().to_lowercase();
        let settlement = self.settlement.to_string().to_lowercase();

        // price from field
        let price_bytes = self.price.to_be_bytes::<32>();

        let mut enc = Vec::with_capacity(32 * 8);
        enc.extend_from_slice(typehash.as_slice());
        enc.extend_from_slice(Self::keccak_string(&self.schema_version).as_slice());
        enc.extend_from_slice(&Self::encode_u256_bytes_be(ts_ns_u));
        enc.extend_from_slice(Self::keccak_string(&system).as_slice());
        enc.extend_from_slice(Self::keccak_string(&network).as_slice());
        enc.extend_from_slice(Self::keccak_string(&settlement).as_slice());
        enc.extend_from_slice(&Self::encode_u256_u64(self.from_block));
        enc.extend_from_slice(&price_bytes);
        keccak256(&enc)
    }

    fn eip712_digest(&self, chain_id: u64) -> B256 {
        let domain_sep = self.domain_separator(chain_id);
        let struct_hash = self.struct_hash();
        let mut buf = Vec::with_capacity(2 + 32 + 32);
        buf.extend_from_slice(&[0x19, 0x01]);
        buf.extend_from_slice(domain_sep.as_slice());
        buf.extend_from_slice(struct_hash.as_slice());
        keccak256(&buf)
    }

    /// EIP-712 signing over the AgentPayload typed data.
    pub fn sign(&self, signer_key: &str) -> Result<String> {
        let signer: PrivateKeySigner = signer_key.parse()?;
        let chain_id =
            SystemNetworkKey::new(self.system.clone(), self.network.clone()).to_chain_id();
        let digest = self.eip712_digest(chain_id);
        let signature = signer.sign_hash_sync(&digest)?;
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

    /// Validates an EIP-712 signature against the payload and returns recovered signer address. Used for round trip test.
    #[cfg(test)]
    pub fn validate_signature(&self, signature: &str) -> Result<Address> {
        let sig_bytes = signature.strip_prefix("0x").unwrap_or(signature);
        let sig_bytes = hex::decode(sig_bytes)?;
        let signature = Signature::from_raw(&sig_bytes)?;
        let chain_id =
            SystemNetworkKey::new(self.system.clone(), self.network.clone()).to_chain_id();
        let digest = self.eip712_digest(chain_id);
        let recovered = signature.recover_address_from_prehash(&digest)?;
        Ok(recovered)
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

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::signers::local::PrivateKeySigner;
    use chrono::{DateTime, Utc};

    #[test]
    fn test_eip712_sign_and_recover_roundtrip() {
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
            price: U256::from(20_000_000_000u128), // 20 gwei in wei
        };

        // Sign
        let sig = payload.sign(sk_hex).unwrap();
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
