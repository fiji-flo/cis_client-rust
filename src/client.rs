use crate::auth::BearerBearer;
use crate::secrets::get_store_from_settings;
use crate::settings::CisSettings;
use cis_profile::crypto::SecretStore;
use cis_profile::schema::Profile;
use condvar_store::CondvarStore;
use percent_encoding::utf8_percent_encode;
use percent_encoding::USERINFO_ENCODE_SET;
use reqwest::Client;
use reqwest::Url;
use serde_json::Value;
use std::sync::Arc;

#[allow(dead_code)]
pub enum GetBy {
    Uuid,
    UserId,
    PrimaryEmail,
    PrimaryUsername,
}

impl GetBy {
    pub fn as_str(self: &GetBy) -> &'static str {
        match self {
            GetBy::Uuid => "uuid/",
            GetBy::UserId => "user_id/",
            GetBy::PrimaryEmail => "primary_email/",
            GetBy::PrimaryUsername => "primary_username/",
        }
    }
}

pub trait CisClientTrait {
    fn get_user_by(&self, id: &str, by: &GetBy, filter: Option<&str>) -> Result<Profile, String>;
    fn update_user(&self, id: &str, profile: Profile) -> Result<Value, String>;
    fn delete_user(&self, id: &str, profile: Profile) -> Result<Value, String>;
    fn get_secret_store(&self) -> &SecretStore;
}

#[derive(Clone)]
pub struct CisClient {
    pub bearer_store: CondvarStore<BearerBearer>,
    pub person_api_user_endpoint: String,
    pub change_api_user_endpoint: String,
    pub secret_store: Arc<SecretStore>,
}

impl CisClient {
    pub fn from_settings(settings: &CisSettings) -> Result<Self, String> {
        let bearer_store = CondvarStore::new(BearerBearer::new(settings.client_config.clone()));
        let secret_store = get_store_from_settings(settings)?;
        Ok(CisClient {
            bearer_store,
            person_api_user_endpoint: settings.person_api_user_endpoint.clone(),
            change_api_user_endpoint: settings.change_api_user_endpoint.clone(),
            secret_store: Arc::new(secret_store),
        })
    }

    fn bearer_token(&self) -> Result<String, String> {
        let b = self
            .bearer_store
            .get()
            .map_err(|e| format!("{}: {}", "unable to get token", e))?;
        let b1 = b
            .read()
            .map_err(|e| format!("{}: {}", "unable to read token", e))?;
        Ok((*b1.bearer_token_str).to_owned())
    }
}

impl CisClientTrait for CisClient {
    fn get_user_by(&self, id: &str, by: &GetBy, filter: Option<&str>) -> Result<Profile, String> {
        let safe_id = utf8_percent_encode(id, USERINFO_ENCODE_SET).to_string();
        let base = Url::parse(&self.person_api_user_endpoint).map_err(|e| format!("{}", e))?;
        let url = base
            .join(by.as_str())
            .and_then(|u| u.join(&safe_id))
            .map(|mut u| {
                if let Some(df) = filter {
                    u.set_query(Some(&format!("filterDisplay={}", df.to_string())))
                }
                u
            })
            .map_err(|e| format!("{}", e))?;
        let token = self.bearer_token()?;
        let client = Client::new().get(url.as_str()).bearer_auth(token);
        let mut res: reqwest::Response = client.send().map_err(|e| format!("{}", e))?;
        if res.status().is_success() {
            res.json()
                .map_err(|e| format!("Invalid JSON from user endpoint: {}", e))
        } else {
            Err(format!("person API returned: {}", res.status()))
        }
    }

    fn update_user(&self, id: &str, profile: Profile) -> Result<Value, String> {
        let safe_id = utf8_percent_encode(id, USERINFO_ENCODE_SET).to_string();
        let token = self.bearer_token()?;
        let mut url = Url::parse(&self.change_api_user_endpoint).map_err(|e| format!("{}", e))?;
        url.set_query(Some(&format!("user_id={}", safe_id)));
        let client = Client::new().post(url).json(&profile).bearer_auth(token);
        let mut res: reqwest::Response = client.send().map_err(|e| format!("change.api: {}", e))?;
        res.json()
            .map_err(|e| format!("change.api → json: {} ({:?})", e, res))
    }

    fn delete_user(&self, id: &str, profile: Profile) -> Result<Value, String> {
        let safe_id = utf8_percent_encode(id, USERINFO_ENCODE_SET).to_string();
        let token = self.bearer_token()?;
        let mut url = Url::parse(&self.change_api_user_endpoint).map_err(|e| format!("{}", e))?;
        url.set_query(Some(&format!("user_id={}", safe_id)));
        let client = Client::new().delete(url).json(&profile).bearer_auth(token);
        let mut res: reqwest::Response = client.send().map_err(|e| format!("change.api: {}", e))?;
        res.json()
            .map_err(|e| format!("change.api → json: {} ({:?})", e, res))
    }

    fn get_secret_store(&self) -> &SecretStore {
        &self.secret_store
    }
}
