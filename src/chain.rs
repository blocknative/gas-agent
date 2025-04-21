use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Chain {
    pub name: String,
    pub rpc: Vec<String>,
    pub native_currency: Currency,
    pub short_name: String,
    pub chain_id: u64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Currency {
    pub name: String,
    pub symbol: String,
    pub decimals: u8,
}
