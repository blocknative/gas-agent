use super::super::types::AgentPayload;
use crate::types::{Network, System, SystemNetworkKey};
use alloy::{
    primitives::aliases::{U240, U48},
    signers::Signature,
};

#[derive(Debug)]
pub struct SignedOraclePayloadV2 {
    pub payload: OraclePayloadV2,
    pub signature: Option<Signature>,
}

#[derive(Debug, Clone)]
pub struct OraclePayloadV2 {
    pub header: OraclePayloadHeaderV2,
    pub records: Vec<OraclePayloadRecordV2>,
}

#[derive(Debug, Clone)]
pub struct OraclePayloadHeaderV2 {
    // Version of the payload format
    pub version: u8,
    // Block height of the payload source
    pub height: u64,
    // ChainID parameter of the record data
    pub chain_id: u64,
    // System ID parameter of the record data
    pub system_id: u8,
    // Timestamp (Miliseconds) of the payload source
    pub timestamp: U48,
    // Number of records in the payload
    pub length: u16,
}

#[derive(Debug, Clone)]
pub struct OraclePayloadRecordV2 {
    // TypeID of the record
    pub typ: u16,
    // Value of the record
    pub value: U240,
}

impl From<AgentPayload> for OraclePayloadV2 {
    fn from(payload: AgentPayload) -> Self {
        let (systemid, chainid) = get_network_config_values(&payload.system, &payload.network);

        OraclePayloadV2 {
            header: OraclePayloadHeaderV2 {
                version: 2,
                height: payload.from_block,
                chain_id: chainid,
                system_id: systemid,
                timestamp: U48::from(payload.timestamp.timestamp_millis()),
                length: 1,
            },
            records: vec![OraclePayloadRecordV2 {
                typ: 340, // Hardcoded into type 340 - Max Priority Fee Per Gas 99th.
                value: {
                    // Convert uint256 price to uint240 by truncating high 16 bits (should be zero for realistic prices)
                    let bytes32 = payload.price.to_be_bytes::<32>();
                    let mut arr30 = [0u8; 30];
                    arr30.copy_from_slice(&bytes32[2..]);
                    U240::from_be_bytes::<30>(arr30)
                },
            }],
        }
    }
}

fn get_network_config_values(system: &System, network: &Network) -> (u8, u64) {
    (
        2,
        SystemNetworkKey::new(system.clone(), network.clone()).to_chain_id(),
    )
}
