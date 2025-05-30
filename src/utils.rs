use alloy::signers::local::PrivateKeySigner;
use anyhow::Result;

pub fn round_to_9_places(v: f64) -> f64 {
    (v * 1_000_000_000.0).round() / 1_000_000_000.0
}

pub fn generate_key_pair() -> Result<()> {
    let signer = PrivateKeySigner::random();

    println!("Private Key: {}", signer.to_bytes());
    println!("Address: {}", signer.address());

    Ok(())
}
