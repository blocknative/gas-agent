use crate::chain::types::{OraclePayloadHeaderV2, OraclePayloadRecordV2, OraclePayloadV2};

pub trait PayloadEncoder {
    fn to_encoded_payload<B>(&self, buf: &mut B) -> usize
    where
        B: bytes::BufMut + AsMut<[u8]>;
}

impl PayloadEncoder for OraclePayloadV2 {
    fn to_encoded_payload<B>(&self, buf: &mut B) -> usize
    where
        B: bytes::BufMut + AsMut<[u8]>,
    {
        let mut size = 0;
        size += self.header.to_encoded_payload(buf);

        for record in &self.records {
            size += record.to_encoded_payload(buf);
        }
        size
    }
}

impl PayloadEncoder for OraclePayloadRecordV2 {
    // Encode record into v2 compatible payload
    // |     | Name   | Type   | Size | Starting Pos. |
    // | --- | ------ | ------ | ---- | -- |
    // | 1   | Value  | uint240|  30  | 0  |
    // | 2   | Type   | uint16 |  2   | 31 |
    fn to_encoded_payload<B>(&self, buf: &mut B) -> usize
    where
        B: bytes::BufMut + AsMut<[u8]>,
    {
        let mut size = 0;
        buf.put_u16(self.typ);
        size += 2;
        buf.put_slice(&self.value.to_be_bytes::<30>());
        size += 30;

        size
    }
}

impl PayloadEncoder for OraclePayloadHeaderV2 {
    // Encode header into v2 compatibale payload
    // |     | Name      | Type   | Size | Starting Pos. |
    // | --- | --------- | ------ | ---- | -- |
    // | 1   | Version   | uint8  |  1   | 32 |
    // | 2   | Height    | uint64 |  8   | 31 |
    // | 3   | ChainID   | uint64 |  8   | 23 |
    // | 4   | SystemID  | uint8  |  1   | 15 |
    // | 5   | Timestamp | uint48 |  6   | 14 |
    // | 6   | Length    | uint16 |  2   | 8  |
    // | 7   | Empty     |        |  0   | 6  |
    fn to_encoded_payload<B>(&self, buf: &mut B) -> usize
    where
        B: bytes::BufMut + AsMut<[u8]>,
    {
        let mut size = 0;
        buf.put_slice(&[0; 6]);
        size += 6;
        buf.put_u16(self.length);
        size += 2;

        buf.put_slice(&self.timestamp.to_be_bytes::<6>());
        size += 6; // U48 is 6 bytes
        buf.put_u8(self.system_id);
        size += 1;
        buf.put_u64(self.chain_id);
        size += 8;
        buf.put_u64(self.height);
        size += 8;
        buf.put_u8(self.version);
        size += 1;
        size
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Once;

    use alloy::primitives::aliases::{U240, U48};

    use super::*;
    use crate::logs::init_logs;

    // This ensures metrics are initialized exactly once
    static INIT: Once = Once::new();

    fn setup() {
        INIT.call_once(|| {
            // Initialize metrics for testing
            init_logs();
        });
    }

    #[test]
    fn test_oracle_payload_record_v2() {
        setup();

        let record = OraclePayloadRecordV2 {
            typ: 0x006c,
            value: U240::from(0x11dab0f6ee_u64),
        };

        let mut buf = Vec::new();
        let size = record.to_encoded_payload(&mut buf);

        assert_eq!(size, 32);
        assert_eq!(buf.len(), 32);

        let encoded =
            hex::decode("006c0000000000000000000000000000000000000000000000000011dab0f6ee")
                .unwrap();
        assert_eq!(buf, encoded);
    }

    #[test]
    fn test_oracle_payload_header_v2() {
        setup();

        let mut buf = Vec::new();
        let header = OraclePayloadHeaderV2 {
            version: 1,
            height: 1236_u64,
            chain_id: 1_u64,
            system_id: 1,
            timestamp: U48::from(1741250000002_u64),
            length: 1,
        };

        let size = header.to_encoded_payload(&mut buf);
        assert_eq!(size, 32);

        let encoded =
            hex::decode("000000000000000101956a96748201000000000000000100000000000004d401")
                .unwrap();
        assert_eq!(buf, encoded);

        //  0x000000000000000101956a96748201000000000000000100000000000004d40100010000000000000000000000000000000000000000000000000000000000061313b7e8cef1bddd87f000f82e289b177bde13b4e7ffaaa39fc27f6be68c353807c4eb1bf5c0c9a6829d0f1a9d369544729febf9ab63fabfc8dd7bc92cda37581b
    }
}
