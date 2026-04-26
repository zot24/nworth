//! Portfolio CLI — command-line client for the REST API.
//!
//! Usage:
//!   nworth-cli [--url http://localhost:8080] <command> [args]
//!
//! Commands:
//!   accounts list                     List all accounts
//!   accounts get <id>                 Get account by ID
//!   accounts create <json>            Create account (JSON body)
//!   accounts update <id> <json>       Update account
//!   accounts delete <id>              Deactivate account
//!
//!   assets list                       List all assets
//!   assets get <id>                   Get asset by ID
//!   assets create <json>              Create asset
//!   assets update <id> <json>         Update asset
//!   assets delete <id>                Deactivate asset
//!
//!   snapshots list [--date YYYY-MM-DD]  List snapshots
//!   snapshots create <json>             Create snapshot
//!   snapshots delete <id>               Delete snapshot
//!   snapshots trigger                   Trigger snapshot from positions
//!
//!   positions list                    List all positions
//!   positions upsert <json>           Create/update position
//!   positions delete <acct_id> <asset_id>
//!
//!   income list                       List all income records
//!   income create <json>              Create income record
//!   income update <id> <json>         Update income record
//!   income delete <id>                Delete income record
//!
//!   expenses list                     List all expense records
//!   expenses create <json>            Create expense record (supports category_id, label_ids)
//!   expenses update <id> <json>       Update expense record
//!   expenses delete <id>              Delete expense record
//!   categories list/create/update/delete   Manage expense categories (parent_id for hierarchy)
//!   labels list/create/update/delete       Manage tags applied to expenses
//!
//!   targets list                      List allocation targets
//!   targets create <json>             Create/upsert target
//!   targets delete <id>               Delete target
//!
//!   networth                          Show current net worth
//!   adjustments                       Show allocation adjustments
//!   apy                               Show stables APY info

use anyhow::{bail, Result};

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();

    // Parse --url flag
    let mut base_url = "http://localhost:8080".to_string();
    let mut cmd_args: Vec<&str> = Vec::new();
    let mut i = 1;
    while i < args.len() {
        if args[i] == "--url" && i + 1 < args.len() {
            base_url = args[i + 1].clone();
            i += 2;
        } else {
            cmd_args.push(&args[i]);
            i += 1;
        }
    }

    if cmd_args.is_empty() {
        print_help();
        return Ok(());
    }

    let client = reqwest::Client::new();
    let entity = cmd_args[0];
    let action = cmd_args.get(1).copied().unwrap_or("list");

    match entity {
        "help" | "--help" | "-h" => {
            print_help();
        }
        "networth" => {
            let resp = client.get(format!("{base_url}/api/networth")).send().await?;
            print_json(resp).await?;
        }
        "adjustments" => {
            let resp = client.get(format!("{base_url}/api/allocation/adjustments")).send().await?;
            print_json(resp).await?;
        }
        "apy" => {
            let resp = client.get(format!("{base_url}/api/stables/apy")).send().await?;
            print_json(resp).await?;
        }
        "accounts" | "assets" | "income" | "expenses" | "targets" | "categories" | "labels" => {
            let path = format!("{base_url}/api/v1/{entity}");
            match action {
                "list" | "ls" => {
                    let resp = client.get(&path).send().await?;
                    print_json(resp).await?;
                }
                "get" => {
                    let id = cmd_args.get(2).ok_or_else(|| anyhow::anyhow!("missing <id>"))?;
                    let resp = client.get(format!("{path}/{id}")).send().await?;
                    print_json(resp).await?;
                }
                "create" | "add" => {
                    let json = cmd_args.get(2).ok_or_else(|| anyhow::anyhow!("missing <json>"))?;
                    let body: serde_json::Value = serde_json::from_str(json)?;
                    let resp = client.post(&path).json(&body).send().await?;
                    print_json(resp).await?;
                }
                "update" | "edit" => {
                    let id = cmd_args.get(2).ok_or_else(|| anyhow::anyhow!("missing <id>"))?;
                    let json = cmd_args.get(3).ok_or_else(|| anyhow::anyhow!("missing <json>"))?;
                    let body: serde_json::Value = serde_json::from_str(json)?;
                    let resp = client.put(format!("{path}/{id}")).json(&body).send().await?;
                    print_json(resp).await?;
                }
                "delete" | "rm" => {
                    let id = cmd_args.get(2).ok_or_else(|| anyhow::anyhow!("missing <id>"))?;
                    let resp = client.delete(format!("{path}/{id}")).send().await?;
                    let status = resp.status();
                    let body = resp.text().await?;
                    if status.is_success() {
                        if body.trim().is_empty() {
                            eprintln!("deleted {entity}/{id}");
                        } else if let Ok(val) = serde_json::from_str::<serde_json::Value>(&body) {
                            println!("{}", serde_json::to_string_pretty(&val)?);
                        } else {
                            println!("{body}");
                        }
                    } else {
                        eprintln!("error {status}: {body}");
                    }
                }
                _ => bail!("unknown action: {action}. Use list/get/create/update/delete"),
            }
        }
        "snapshots" => {
            let path = format!("{base_url}/api/v1/snapshots");
            match action {
                "list" | "ls" => {
                    let date = cmd_args.get(2).and_then(|a| {
                        if *a == "--date" { cmd_args.get(3).copied() } else { Some(*a) }
                    });
                    let url = if let Some(d) = date {
                        format!("{path}?as_of={d}")
                    } else {
                        path.clone()
                    };
                    let resp = client.get(&url).send().await?;
                    print_json(resp).await?;
                }
                "create" | "add" => {
                    let json = cmd_args.get(2).ok_or_else(|| anyhow::anyhow!("missing <json>"))?;
                    let body: serde_json::Value = serde_json::from_str(json)?;
                    let resp = client.post(&path).json(&body).send().await?;
                    print_json(resp).await?;
                }
                "delete" | "rm" => {
                    let id = cmd_args.get(2).ok_or_else(|| anyhow::anyhow!("missing <id>"))?;
                    let resp = client.delete(format!("{path}/{id}")).send().await?;
                    if resp.status().is_success() {
                        eprintln!("deleted snapshot {id}");
                    } else {
                        eprintln!("error: {}", resp.status());
                    }
                }
                "trigger" => {
                    let resp = client.post(format!("{path}/trigger")).send().await?;
                    print_json(resp).await?;
                }
                _ => bail!("unknown action: {action}. Use list/create/delete/trigger"),
            }
        }
        "positions" => {
            let path = format!("{base_url}/api/v1/positions");
            match action {
                "list" | "ls" => {
                    let resp = client.get(&path).send().await?;
                    print_json(resp).await?;
                }
                "upsert" | "create" | "add" => {
                    let json = cmd_args.get(2).ok_or_else(|| anyhow::anyhow!("missing <json>"))?;
                    let body: serde_json::Value = serde_json::from_str(json)?;
                    let resp = client.post(&path).json(&body).send().await?;
                    print_json(resp).await?;
                }
                "delete" | "rm" => {
                    let acct = cmd_args.get(2).ok_or_else(|| anyhow::anyhow!("missing <acct_id>"))?;
                    let asset = cmd_args.get(3).ok_or_else(|| anyhow::anyhow!("missing <asset_id>"))?;
                    let resp = client.delete(format!("{path}/{acct}/{asset}")).send().await?;
                    if resp.status().is_success() {
                        eprintln!("deleted position {acct}/{asset}");
                    } else {
                        eprintln!("error: {}", resp.status());
                    }
                }
                _ => bail!("unknown action: {action}. Use list/upsert/delete"),
            }
        }
        _ => bail!("unknown entity: {entity}. Use accounts/assets/snapshots/positions/income/expenses/targets/categories/labels/networth/adjustments/apy"),
    }

    Ok(())
}

async fn print_json(resp: reqwest::Response) -> Result<()> {
    let status = resp.status();
    let body = resp.text().await?;
    if status.is_success() {
        // Pretty-print JSON
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&body) {
            println!("{}", serde_json::to_string_pretty(&val)?);
        } else {
            println!("{body}");
        }
    } else {
        eprintln!("error {status}: {body}");
    }
    Ok(())
}

fn print_help() {
    eprintln!("nworth-cli — command-line client for the nworth-web API");
    eprintln!();
    eprintln!("Usage: nworth-cli [--url URL] <entity> <action> [args]");
    eprintln!();
    eprintln!("Entities & actions:");
    eprintln!("  accounts   list | get <id> | create <json> | update <id> <json> | delete <id>");
    eprintln!("  assets     list | get <id> | create <json> | update <id> <json> | delete <id>");
    eprintln!("  snapshots  list [date] | create <json> | delete <id> | trigger");
    eprintln!("  positions  list | upsert <json> | delete <acct_id> <asset_id>");
    eprintln!("  income     list | create <json> | update <id> <json> | delete <id>");
    eprintln!("  expenses   list | create <json> | update <id> <json> | delete <id>");
    eprintln!("  targets    list | create <json> | delete <id>");
    eprintln!("  categories list | get <id> | create <json> | update <id> <json> | delete <id>");
    eprintln!("  labels     list | get <id> | create <json> | update <id> <json> | delete <id>");
    eprintln!("  networth   (show current net worth)");
    eprintln!("  adjustments (show allocation adjustments)");
    eprintln!("  apy        (show stables APY)");
    eprintln!();
    eprintln!("Options:");
    eprintln!("  --url URL  API base URL (default: http://localhost:8080)");
    eprintln!();
    eprintln!("Examples:");
    eprintln!("  nworth-cli accounts list");
    eprintln!("  nworth-cli assets create '{{\"symbol\":\"AAPL\",\"type_code\":\"stock\"}}'");
    eprintln!("  nworth-cli income create '{{\"as_of\":\"2026-04-01\",\"salary_usd\":8000}}'");
    eprintln!("  nworth-cli categories create '{{\"name\":\"Food\"}}'");
    eprintln!("  nworth-cli categories create '{{\"name\":\"Restaurants\",\"parent_id\":1,\"color\":\"#c0392b\"}}'");
    eprintln!("  nworth-cli labels create '{{\"name\":\"vacation-2026\"}}'");
    eprintln!("  nworth-cli expenses create '{{\"as_of\":\"2026-04-26\",\"amount_usd\":42.5,\"place\":\"Sushi-X\",\"category_id\":2,\"label_ids\":[1]}}'");
    eprintln!("  nworth-cli snapshots trigger");
    eprintln!("  nworth-cli networth");
}
