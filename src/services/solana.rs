//! Solana on-chain balance reads via Helius RPC.
//! Stub — expand once we pick Helius plan (or switch to public RPC for reads).

use anyhow::Result;
use serde_json::json;

pub async fn get_sol_balance(
    client: &reqwest::Client,
    rpc_url: &str,
    address: &str,
) -> Result<f64> {
    let body = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "getBalance",
        "params": [address]
    });
    let resp: serde_json::Value = client
        .post(rpc_url)
        .json(&body)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let lamports = resp
        .pointer("/result/value")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    Ok(lamports as f64 / 1_000_000_000.0)
}

pub async fn get_token_accounts(
    client: &reqwest::Client,
    rpc_url: &str,
    address: &str,
) -> Result<serde_json::Value> {
    let body = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "getTokenAccountsByOwner",
        "params": [
            address,
            {"programId": "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"},
            {"encoding": "jsonParsed"}
        ]
    });
    let resp: serde_json::Value = client
        .post(rpc_url)
        .json(&body)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    Ok(resp)
}
