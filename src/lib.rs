use std::error::Error;
mod transaction;
use self::transaction::{Decodable, Transaction};

pub fn decode(transaction_hex: String) -> Result<String, Box<dyn Error>> {
    let transaction_bytes = hex::decode(transaction_hex).map_err(|e| format!("Hex decode error: {}", e))?;
    let transaction = Transaction::consensus_decode(&mut transaction_bytes.as_slice())?;
    Ok(serde_json::to_string_pretty(&transaction)?)
}
