use biscuit::jws;
use chrono::DateTime;
use chrono::TimeZone;
use chrono::Utc;
use reqwest::Client;

use serde_json::Value;

use crate::settings::ClientConfig;
use condvar_store::GetExpiry;

pub struct BearerBearer {
    pub bearer_token_str: String,
    pub exp: DateTime<Utc>,
    pub config: ClientConfig,
}

impl GetExpiry for BearerBearer {
    fn get(&mut self) -> Result<(), String> {
        self.bearer_token_str = get_raw_access_token(&self.config)?;
        self.exp = get_expiration(&self.bearer_token_str)?;
        Ok(())
    }
    fn expiry(&self) -> DateTime<Utc> {
        self.exp
    }
}

impl BearerBearer {
    pub fn new(config: ClientConfig) -> Self {
        BearerBearer {
            bearer_token_str: String::default(),
            exp: Utc.timestamp(0, 0),
            config,
        }
    }
}

fn get_expiration(token: &str) -> Result<DateTime<Utc>, String> {
    let c: jws::Compact<biscuit::ClaimsSet<Value>, biscuit::Empty> =
        jws::Compact::new_encoded(&token);
    let payload = c
        .unverified_payload()
        .map_err(|e| format!("unable to get payload from token: {}", e))?;
    let exp = payload
        .registered
        .expiry
        .ok_or_else(|| String::from("no expiration set in token"))?;
    Ok(*exp)
}

pub fn get_raw_access_token(client_config: &ClientConfig) -> Result<String, String> {
    let payload = json!(
        {
            "client_id": client_config.client_id,
            "client_secret": client_config.client_secret,
            "audience": client_config.audience,
            "grant_type": "client_credentials",
            "scopes": client_config.scopes,
        }
    );
    let client = Client::new();
    let mut res = client
        .post(&client_config.token_endpoint)
        .json(&payload)
        .send()
        .map_err(|e| format!("can't get token: {}", e))?;
    let j: serde_json::Value = res
        .json()
        .map_err(|e| format!("can't parse token: {}", e))?;
    j["access_token"]
        .as_str()
        .map(|s| s.to_owned())
        .ok_or_else(|| String::from("no token :/"))
}
