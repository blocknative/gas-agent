use crate::chain::{encode::PayloadEncoder, types::SignedOraclePayloadV2};
use alloy::{primitives::keccak256, signers::SignerSync};
use bytes::BufMut;

pub trait PayloadSigner {
    fn to_signed_payload<B, S>(&mut self, buf: &mut B, signer: S) -> Result<usize, SignerError>
    where
        B: bytes::BufMut + AsMut<[u8]>,
        S: SignerSync;
}

impl PayloadSigner for SignedOraclePayloadV2 {
    fn to_signed_payload<B, S>(&mut self, buf: &mut B, signer: S) -> Result<usize, SignerError>
    where
        B: bytes::BufMut + AsMut<[u8]>,
        S: SignerSync,
    {
        let mut buf_int = vec![];
        let mut size = 0;
        size += self.payload.to_encoded_payload(&mut buf_int);

        // sign the keccak256 hash, not the payload
        match signer.sign_hash_sync(&keccak256(&buf_int)) {
            Ok(signature) => {
                self.signature = Some(signature);
            }
            Err(e) => {
                return Err(SignerError::SigningError(e.to_string()));
            }
        };
        let sig_bytes = &self.signature.unwrap().as_bytes();
        buf_int.put_slice(sig_bytes);
        size += sig_bytes.len();
        buf.put_slice(&buf_int);
        Ok(size)
    }
}

#[derive(Clone, Debug)]
pub enum SignerError {
    SigningError(String),
}

impl std::error::Error for SignerError {}

impl std::fmt::Display for SignerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SignerError::SigningError(msg) => write!(f, "Signing Error: {}", msg),
        }
    }
}

#[cfg(test)]
mod tests {
    use alloy::{
        primitives::aliases::{U240, U48},
        signers::local::PrivateKeySigner,
    };

    use super::*;
    use crate::chain::types::{OraclePayloadHeaderV2, OraclePayloadRecordV2, OraclePayloadV2};

    #[test]
    fn test_to_signed_payload_success() {
        let mut payload = SignedOraclePayloadV2 {
            payload: OraclePayloadV2 {
                header: OraclePayloadHeaderV2 {
                    height: 1234,
                    chain_id: 56789,
                    system_id: 2,
                    version: 1,
                    timestamp: U48::from(1234567890),
                    length: 1,
                },
                records: vec![OraclePayloadRecordV2 {
                    typ: 234,
                    value: U240::from(1234567890),
                }],
            },
            signature: None,
        };

        let mut buf = vec![];

        let signer = PrivateKeySigner::random();
        let initial_signer = signer.address();
        let result = payload.to_signed_payload(&mut buf, signer);

        assert!(result.is_ok());
        assert_eq!(buf.len(), 129);
        assert!(!payload.signature.unwrap().as_bytes().is_empty());

        let recovered = payload
            .signature
            .unwrap()
            .recover_address_from_prehash(&keccak256(&buf[0..64]))
            .unwrap();
        assert_eq!(recovered, initial_signer);
    }
}
