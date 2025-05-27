// use crate::chain::types::{OraclePayloadHeaderV2, OraclePayloadRecordV2, OraclePayloadV2};
// use alloy_primitives::aliases::{U240, U48};
// use shared::types::AgentPayload;

// 
// pub trait OracleConverter<A> {
//     fn convert(&self, payload: A) -> OraclePayloadV2;
// }

// impl OracleConverter<&AgentPayload> for PayloadConverter {
//     fn convert(&self, payload: &AgentPayload) -> OraclePayloadV2 {
//         let (systemid, chainid) =
//             self.get_network_config_values(&payload.system, &payload.network, &payload.chain_id);
//         OraclePayloadV2 {
//             header: OraclePayloadHeaderV2 {
//                 version: 2,
//                 height: payload.from_block,
//                 chain_id: chainid,
//                 system_id: systemid,
//                 timestamp: U48::from(payload.timestamp.timestamp_millis()),
//                 length: 1 as u16,
//             },
//             records: vec![OraclePayloadRecordV2 {
//                 typ: 340,
//                 value: U240::from(payload.price),
//             }],
//         }
//     }
// }

// pub struct PayloadConverter;

// impl PayloadConverter {
//     fn get_network_config_values(&self, chain: &str, system: &str, chain_id: &u64) -> (u8, u64) {
//         match chain {
//             "bitcoin" => match system {
//                 "mainnet" => (1, 1),
//                 _ => (1, chain_id.clone()),
//             },
//             _ => (2, chain_id.clone()),
//         }
//     }
// }

// #[cfg(test)]
// mod tests {

//     use super::*;
//     use chrono::Utc;
//     use shared::types::{AgentPayloadKind, FeeUnit, Settlement};

//     #[test]
//     fn test_oracle_payload_v2() {
//         let payload = AgentPayload {
//             from_block: 123456,
//             settlement: Settlement::Fast,
//             timestamp: Utc::now(),
//             unit: FeeUnit::Gwei,
//             system: "ethereum".to_string(),
//             chain_id: 1,
//             network: "mainnet".to_string(),
//             price: 987654321.0,
//             kind: AgentPayloadKind::Estimate,
//         };

//         let converter = PayloadConverter {};
//         let converted_payload = converter.convert(&payload);

//         assert_eq!(converted_payload.header.height, 123456);

//         assert_eq!(
//             converted_payload.header.timestamp,
//             U48::from(payload.timestamp.timestamp_millis())
//         );

//         assert_eq!(converted_payload.header.length, 1);

//         assert_eq!(converted_payload.header.version, 2);

//         assert_eq!(converted_payload.header.system_id, 2);

//         assert_eq!(converted_payload.header.chain_id, 1);
//     }
// }
