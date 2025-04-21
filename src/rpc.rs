use anyhow::{anyhow, Ok, Result};
use chrono::{DateTime, TimeZone, Utc};
use rand::Rng;
use reqwest::{Client, Url};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fmt::Debug;

use crate::blocks::wei_to_gwei;

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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Block {
    pub number: u64,
    pub timestamp: DateTime<Utc>,
    pub gas_limit: u64,
    pub gas_used: u64,
    pub base_fee_per_gas: Option<u64>,
    pub transactions: Vec<Transaction>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Transaction {
    pub hash: String,
    pub gas_price: Option<u128>,
    pub max_fee_per_gas: Option<u128>,
    pub max_priority_fee_per_gas: Option<u128>,
}

pub fn get_rpc_client(rpc_url: Url) -> RpcClient {
    let client = RpcClient::new(rpc_url.to_string());
    client
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
        .map_err(|e| dbg!(e, cleaned))
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
    let transactions = if let Some(txs_array) = value["transactions"].as_array() {
        txs_array
            .iter()
            .map(|tx| {
                let tx_hash = tx["hash"]
                    .as_str()
                    .ok_or(anyhow!("Missing or invalid transaction hash"))?
                    .to_string();

                // All fee fields are optional in our struct
                let gas_price = tx["gasPrice"].as_str().map(parse_hex_to_u128);

                let max_fee_per_gas = tx["maxFeePerGas"].as_str().map(parse_hex_to_u128);

                let max_priority_fee_per_gas =
                    tx["maxPriorityFeePerGas"].as_str().map(parse_hex_to_u128);

                Ok(Transaction {
                    hash: tx_hash,
                    gas_price,
                    max_fee_per_gas,
                    max_priority_fee_per_gas,
                })
            })
            .collect::<Result<Vec<Transaction>>>()?
    } else {
        return Err(anyhow!("Missing or invalid transactions array"));
    };

    Ok(Block {
        number,
        timestamp,
        gas_used,
        gas_limit,
        base_fee_per_gas,
        transactions,
    })
}
