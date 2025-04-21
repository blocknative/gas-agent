use alloy::signers::local::PrivateKeySigner;
use reqwest::Client;
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

use crate::chain::Chain;

pub fn extract_system_network(val: &str) -> (String, String) {
    let parts: Vec<&str> = val.split(' ').collect();

    let system = if parts.len() > 0 { parts[0] } else { &val };
    let network = parts.get(1).unwrap_or(&"Mainnet");

    (
        system.to_string().to_lowercase(),
        network.to_string().to_lowercase(),
    )
}

pub fn round_to_9_places(v: f64) -> f64 {
    (v * 1_000_000_000.0).round() / 1_000_000_000.0
}

pub fn generate_random_signer_key() -> String {
    // Generate a random private key on the secp256k1 curve
    let private_key = PrivateKeySigner::random().as_nonzero_scalar().to_string();

    // Convert to hex string (without 0x prefix)
    let key = private_key
        .strip_prefix("0x")
        .unwrap_or(&private_key)
        .to_string();

    key
}

pub fn get_or_create_signer_key() -> String {
    let signer_key_path = Path::new("signer_key");

    // Try to read the existing key file
    if signer_key_path.exists() {
        let mut file = match File::open(signer_key_path) {
            Ok(file) => file,
            Err(e) => {
                eprintln!("Failed to open signer_key file: {}", e);
                return generate_random_signer_key();
            }
        };

        let mut key = String::new();
        if let Err(e) = file.read_to_string(&mut key) {
            eprintln!("Failed to read signer_key file: {}", e);
            return generate_random_signer_key();
        }

        // Trim any whitespace or newlines
        key.trim().to_string()
    } else {
        // Generate a new key
        let key = generate_random_signer_key();

        // Save the key to a file
        let mut file = match File::create(signer_key_path) {
            Ok(file) => file,
            Err(e) => {
                eprintln!("Failed to create signer_key file: {}", e);
                return key;
            }
        };

        if let Err(e) = file.write_all(key.as_bytes()) {
            eprintln!("Failed to write to signer_key file: {}", e);
        }

        key
    }
}

pub async fn load_chain_list() -> anyhow::Result<Vec<Chain>> {
    let client = Client::new();
    let response = client
        .get("https://chainid.network/chains_mini.json")
        .send()
        .await?;

    let json = response.json().await?;
    let chains: Vec<Chain> = serde_json::from_value(json)?;
    Ok(chains)
}
