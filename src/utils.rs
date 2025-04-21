// use alloy::signers::local::PrivateKeySigner;

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

// pub fn generate_random_signer_key() -> String {
//     // Generate a random private key on the secp256k1 curve
//     let private_key = PrivateKeySigner::random().as_nonzero_scalar().to_string();

//     // Convert to hex string (without 0x prefix)
//     let key = private_key
//         .strip_prefix("0x")
//         .unwrap_or(&private_key)
//         .to_string();

//     key
// }
