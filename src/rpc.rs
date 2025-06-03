use anyhow::{anyhow, Ok, Result};
use chrono::{DateTime, TimeZone, Utc};
use rand::Rng;
use reqwest::{Client, Url};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fmt::Debug;
use tracing::error;

use crate::blocks::wei_to_gwei;

#[derive(Clone)]
pub struct RpcClient {
    host: String,
    client: Client,
}

impl RpcClient {
    pub fn new(host: String) -> Self {
        RpcClient {
            host,
            client: Client::new(),
        }
    }

    pub async fn request<T>(&self, request: &Request) -> Result<T, RpcError>
    where
        T: for<'de> Deserialize<'de> + Debug,
    {
        let response: Response<T> = self
            .client
            .post(&self.host)
            .json(request)
            .send()
            .await
            .map_err(|e| RpcError {
                code: -32603,
                message: e.to_string(),
                data: None,
            })?
            .json()
            .await
            .map_err(|e| RpcError {
                code: -32603,
                message: e.to_string(),
                data: None,
            })?;

        if let Some(error) = response.error {
            Err(error)
        } else {
            response.result.ok_or(RpcError {
                code: -32603,
                message: "No result in response".to_string(),
                data: None,
            })
        }
    }

    pub fn create_request(&self, method: &str, params: Option<Value>) -> Request {
        Request {
            method: method.to_string(),
            params,
            id: json!(generate_rpc_id()),
            jsonrpc: Some("2.0".to_string()),
        }
    }

    pub async fn get_latest_block(&self) -> Result<Block> {
        let value: Value = self
            .request(&self.create_request("eth_getBlockByNumber", Some(json!(["latest", true]))))
            .await?;

        let block = parse_block(&value)?;

        Ok(block)
    }

    pub async fn get_pending_block(
        &self,
        method: &str,
        params: Option<Value>,
    ) -> Result<Vec<Transaction>> {
        let value: Value = self.request(&self.create_request(method, params)).await?;

        let transactions = parse_transactions(&value)?;

        Ok(transactions)
    }

    pub async fn get_chain_id(&self) -> Result<u64> {
        let value: Value = self
            .request(&self.create_request("eth_chainId", None))
            .await?;

        let hex = value.as_str().unwrap().to_string();
        let chain_id = parse_hex_to_u64(&hex);

        Ok(chain_id)
    }

    pub async fn get_node_gas_price_estimate(&self) -> Result<f64> {
        let value: Value = self
            .request(&self.create_request("eth_gasPrice", None))
            .await?;

        let hex = value.as_str().unwrap().to_string();
        let wei = parse_hex_to_u128(&hex);
        let gwei = wei_to_gwei(wei)?;

        Ok(gwei)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Request {
    /// The name of the RPC call.
    pub method: String,
    /// Parameters to the RPC call.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
    /// Identifier for this request, which should appear in the response.
    pub id: Value,
    /// jsonrpc field, MUST be "2.0".
    pub jsonrpc: Option<String>,
}

/// A JSONRPC response object.
#[derive(Debug, Clone, Deserialize)]
pub struct Response<T> {
    /// A result if there is one, or [`None`].
    pub result: Option<T>,
    /// An error if there is one, or [`None`].
    pub error: Option<RpcError>,
    // /// Identifier for this response, which should match that of the request.
    // pub id: Value,
    // /// jsonrpc field, MUST be "2.0".
    // pub jsonrpc: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RpcError {
    /// The integer identifier of the error
    pub code: i32,
    /// A string describing the error
    pub message: String,
    /// Additional data specific to the error
    pub data: Option<Value>,
}

impl std::error::Error for RpcError {}

impl std::fmt::Display for RpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "RPC Error {}: {} {}",
            self.code,
            self.message,
            self.data
                .as_ref()
                .map(|d| d.to_string())
                .unwrap_or_default()
        )
    }
}

fn generate_rpc_id() -> u32 {
    rand::rng().random()
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Block {
    pub number: u64,
    pub timestamp: DateTime<Utc>,
    pub gas_limit: u64,
    pub gas_used: u64,
    pub base_fee_per_gas: Option<u64>,
    pub transactions: Vec<Transaction>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BlockHeader {
    pub number: u64,
    pub timestamp: DateTime<Utc>,
    pub gas_limit: u64,
    pub gas_used: u64,
    pub base_fee_per_gas: Option<u64>,
}

impl From<Block> for BlockHeader {
    fn from(block: Block) -> Self {
        BlockHeader {
            number: block.number,
            timestamp: block.timestamp,
            gas_limit: block.gas_limit,
            gas_used: block.gas_used,
            base_fee_per_gas: block.base_fee_per_gas,
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Transaction {
    pub hash: String,
    pub gas_price: Option<u128>,
    pub max_fee_per_gas: Option<u128>,
    pub max_priority_fee_per_gas: Option<u128>,
}

pub fn get_rpc_client(rpc_url: Url) -> RpcClient {
    RpcClient::new(rpc_url.to_string())
}

pub async fn get_latest_block(client: &RpcClient) -> Result<Block> {
    let block = client.get_latest_block().await?;
    Ok(block)
}

fn parse_hex_to_u64(hex_str: &str) -> u64 {
    let cleaned = hex_str.strip_prefix("0x").unwrap_or(hex_str);
    u64::from_str_radix(cleaned, 16).unwrap_or(0)
}

fn parse_hex_to_u128(hex_str: &str) -> u128 {
    let cleaned = hex_str.strip_prefix("0x").unwrap_or(hex_str);
    u128::from_str_radix(cleaned, 16)
        .map_err(|e| error!("Failed to parse hex to u128: {}", e))
        .unwrap_or(0)
}

pub fn parse_block(value: &Value) -> Result<Block> {
    // Parse the number field (hex string to u64)
    let number_hex = value["number"]
        .as_str()
        .ok_or(anyhow!("Missing or invalid number field"))?;

    let number = parse_hex_to_u64(number_hex);

    // Parse the timestamp field (hex string to u64, then to DateTime<Utc>)
    let timestamp_hex = value["timestamp"]
        .as_str()
        .ok_or(anyhow!("Missing or invalid timestamp field"))?;

    let timestamp_secs = parse_hex_to_u64(timestamp_hex);
    let timestamp = Utc.timestamp_opt(timestamp_secs as i64, 0).unwrap();

    // Parse the baseFeePerGas field (optional)
    let base_fee_per_gas = value["baseFeePerGas"].as_str().map(parse_hex_to_u64);

    let gas_used = value["gasUsed"]
        .as_str()
        .map(parse_hex_to_u64)
        .ok_or(anyhow!("Missing or invalid gasUsed field"))?;

    let gas_limit = value["gasLimit"]
        .as_str()
        .map(parse_hex_to_u64)
        .ok_or(anyhow!("Missing or invalid gasLimit field"))?;

    // Parse transactions
    let transactions = parse_transactions(value)?;

    Ok(Block {
        number,
        timestamp,
        gas_used,
        gas_limit,
        base_fee_per_gas,
        transactions,
    })
}

fn parse_transactions(block: &Value) -> Result<Vec<Transaction>> {
    if let Some(txs_array) = block["transactions"].as_array() {
        txs_array
            .iter()
            .map(|tx| {
                let tx_hash = tx["hash"]
                    .as_str()
                    .ok_or(anyhow!("Missing or invalid transaction hash"))?
                    .to_string();

                // Parse fee fields
                let gas_price = tx["gasPrice"].as_str().map(parse_hex_to_u128);
                let max_fee_per_gas = tx["maxFeePerGas"].as_str().map(parse_hex_to_u128);
                let max_priority_fee_per_gas =
                    tx["maxPriorityFeePerGas"].as_str().map(parse_hex_to_u128);

                // Validate gas pricing: either gas_price OR (max_fee_per_gas AND max_priority_fee_per_gas)
                let has_legacy_pricing = gas_price.is_some();
                let has_eip1559_pricing = max_fee_per_gas.is_some() && max_priority_fee_per_gas.is_some();

                if !has_legacy_pricing && !has_eip1559_pricing {
                    return Err(anyhow!(
                        "Transaction {} missing valid gas pricing: must have either gasPrice or both maxFeePerGas and maxPriorityFeePerGas",
                        tx_hash
                    ));
                }

                Ok(Transaction {
                    hash: tx_hash,
                    gas_price,
                    max_fee_per_gas,
                    max_priority_fee_per_gas,
                })
            })
            .collect::<Result<Vec<Transaction>>>()
    } else {
        Err(anyhow!("Missing or invalid transactions array"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_parse_transactions_with_legacy_pricing() {
        let block_data = json!({
            "transactions": [
                {
                    "hash": "0x1234567890abcdef",
                    "gasPrice": "0x12a05f200"
                }
            ]
        });

        let result = parse_transactions(&block_data).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].hash, "0x1234567890abcdef");
        assert_eq!(result[0].gas_price, Some(5000000000));
        assert_eq!(result[0].max_fee_per_gas, None);
        assert_eq!(result[0].max_priority_fee_per_gas, None);
    }

    #[test]
    fn test_parse_transactions_with_eip1559_pricing() {
        let block_data = json!({
            "transactions": [
                {
                    "hash": "0xabcdef1234567890",
                    "maxFeePerGas": "0x174876e800",
                    "maxPriorityFeePerGas": "0x3b9aca00"
                }
            ]
        });

        let result = parse_transactions(&block_data).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].hash, "0xabcdef1234567890");
        assert_eq!(result[0].gas_price, None);
        assert_eq!(result[0].max_fee_per_gas, Some(100000000000));
        assert_eq!(result[0].max_priority_fee_per_gas, Some(1000000000));
    }

    #[test]
    fn test_parse_transactions_with_both_pricing_types() {
        let block_data = json!({
            "transactions": [
                {
                    "hash": "0x1111111111111111",
                    "gasPrice": "0x12a05f200",
                    "maxFeePerGas": "0x174876e800",
                    "maxPriorityFeePerGas": "0x3b9aca00"
                }
            ]
        });

        let result = parse_transactions(&block_data).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].hash, "0x1111111111111111");
        assert_eq!(result[0].gas_price, Some(5000000000));
        assert_eq!(result[0].max_fee_per_gas, Some(100000000000));
        assert_eq!(result[0].max_priority_fee_per_gas, Some(1000000000));
    }

    #[test]
    fn test_parse_transactions_missing_gas_price() {
        let block_data = json!({
            "transactions": [
                {
                    "hash": "0x2222222222222222"
                }
            ]
        });

        let result = parse_transactions(&block_data);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("missing valid gas pricing"));
    }

    #[test]
    fn test_parse_transactions_incomplete_eip1559_pricing() {
        let block_data = json!({
            "transactions": [
                {
                    "hash": "0x3333333333333333",
                    "maxFeePerGas": "0x174876e800"
                    // Missing maxPriorityFeePerGas
                }
            ]
        });

        let result = parse_transactions(&block_data);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("missing valid gas pricing"));
    }

    #[test]
    fn test_parse_transactions_multiple_valid_transactions() {
        let block_data = json!({
            "transactions": [
                {
                    "hash": "0x4444444444444444",
                    "gasPrice": "0x12a05f200"
                },
                {
                    "hash": "0x5555555555555555",
                    "maxFeePerGas": "0x174876e800",
                    "maxPriorityFeePerGas": "0x3b9aca00"
                }
            ]
        });

        let result = parse_transactions(&block_data).unwrap();
        assert_eq!(result.len(), 2);
        
        // First transaction (legacy)
        assert_eq!(result[0].hash, "0x4444444444444444");
        assert_eq!(result[0].gas_price, Some(5000000000));
        assert_eq!(result[0].max_fee_per_gas, None);
        assert_eq!(result[0].max_priority_fee_per_gas, None);
        
        // Second transaction (EIP-1559)
        assert_eq!(result[1].hash, "0x5555555555555555");
        assert_eq!(result[1].gas_price, None);
        assert_eq!(result[1].max_fee_per_gas, Some(100000000000));
        assert_eq!(result[1].max_priority_fee_per_gas, Some(1000000000));
    }

    #[test]
    fn test_parse_transactions_missing_hash() {
        let block_data = json!({
            "transactions": [
                {
                    "gasPrice": "0x12a05f200"
                    // Missing hash
                }
            ]
        });

        let result = parse_transactions(&block_data);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Missing or invalid transaction hash"));
    }

    #[test]
    fn test_parse_transactions_empty_array() {
        let block_data = json!({
            "transactions": []
        });

        let result = parse_transactions(&block_data).unwrap();
        assert_eq!(result.len(), 0);
    }

    #[test]
    fn test_parse_transactions_missing_transactions_field() {
        let block_data = json!({
            "number": "0x1"
        });

        let result = parse_transactions(&block_data);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Missing or invalid transactions array"));
    }
}
