//! Minimal Supabase client: auth (password + refresh) and PostgREST upsert/select.

use std::time::Duration;

use reqwest::blocking::Client as Http;
use serde::Deserialize;
use serde_json::Value;

pub struct Client {
    http: Http,
    url: String,
    anon: String,
}

#[derive(Debug, Clone)]
pub struct Tokens {
    pub access: String,
    pub refresh: String,
    pub expires_at: i64,
    pub user_id: String,
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: String,
    expires_at: Option<i64>,
    expires_in: Option<i64>,
    user: TokenUser,
}
#[derive(Deserialize)]
struct TokenUser {
    id: String,
}

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn body_error(status: reqwest::StatusCode, text: String) -> String {
    // A table or column this version uses does not exist in Supabase yet.
    if ["PGRST204", "PGRST205", "42703", "42P01"].iter().any(|code| text.contains(code)) {
        return "Supabase needs the latest setup. In Supabase open SQL Editor, paste supabase/schema.sql and run it. \
                Nothing is lost: changes wait on this computer until then."
            .into();
    }
    let short: String = text.chars().take(300).collect();
    format!("HTTP {status}: {short}")
}

impl Client {
    pub fn new(url: &str, anon: &str) -> Result<Self, String> {
        let http = Http::builder()
            .timeout(Duration::from_secs(20))
            .build()
            .map_err(|e| e.to_string())?;
        Ok(Self { http, url: url.trim_end_matches('/').to_string(), anon: anon.to_string() })
    }

    fn token_call(&self, grant: &str, body: Value) -> Result<Tokens, String> {
        let res = self
            .http
            .post(format!("{}/auth/v1/token?grant_type={grant}", self.url))
            .header("apikey", &self.anon)
            .json(&body)
            .send()
            .map_err(|e| format!("Could not reach Supabase: {e}"))?;
        let status = res.status();
        let text = res.text().map_err(|e| e.to_string())?;
        if !status.is_success() {
            return Err(body_error(status, text));
        }
        let t: TokenResponse = serde_json::from_str(&text).map_err(|e| format!("Bad auth response: {e}"))?;
        let expires_at = t.expires_at.unwrap_or_else(|| now() + t.expires_in.unwrap_or(3600));
        Ok(Tokens { access: t.access_token, refresh: t.refresh_token, expires_at, user_id: t.user.id })
    }

    /// Confirms the URL really is a Supabase project before we try to log in,
    /// so a pasted dashboard link or a paused project gets a clear message.
    pub fn check_project(&self) -> Result<(), String> {
        let res = self
            .http
            .get(format!("{}/auth/v1/health", self.url))
            .header("apikey", &self.anon)
            .send()
            .map_err(|e| format!("Could not reach {}: {e}", self.url))?;
        match res.status().as_u16() {
            200 => Ok(()),
            404 => Err(format!(
                "{} is not a Supabase project URL. It should look like https://abcdefghijkl.supabase.co \
                 (Project Settings > API > Project URL), not the dashboard link.",
                self.url
            )),
            401 | 403 => Err("The anon key was rejected. Copy the anon public key from Project Settings > API.".into()),
            540 | 503 => Err("The Supabase project is paused. Open it in the dashboard and click Restore.".into()),
            code => Err(format!("Supabase answered HTTP {code} at {}/auth/v1/health", self.url)),
        }
    }

    pub fn password_login(&self, email: &str, password: &str) -> Result<Tokens, String> {
        self.token_call("password", serde_json::json!({ "email": email, "password": password }))
    }

    pub fn refresh(&self, refresh_token: &str) -> Result<Tokens, String> {
        self.token_call("refresh_token", serde_json::json!({ "refresh_token": refresh_token }))
    }

    /// Upsert rows by `uid`. Rows in one call must have distinct uids.
    pub fn upsert(&self, token: &str, table: &str, rows: &[Value]) -> Result<(), String> {
        self.upsert_on(token, table, "uid", rows)
    }

    pub fn upsert_on(&self, token: &str, table: &str, key: &str, rows: &[Value]) -> Result<(), String> {
        let res = self
            .http
            .post(format!("{}/rest/v1/{table}?on_conflict={key}", self.url))
            .header("apikey", &self.anon)
            .bearer_auth(token)
            .header("Prefer", "resolution=merge-duplicates,return=minimal")
            .json(rows)
            .send()
            .map_err(|e| format!("Could not reach Supabase: {e}"))?;
        let status = res.status();
        if status.is_success() {
            return Ok(());
        }
        Err(body_error(status, res.text().unwrap_or_default()))
    }

    pub fn select(&self, token: &str, path_and_query: &str) -> Result<Vec<Value>, String> {
        let res = self
            .http
            .get(format!("{}/rest/v1/{path_and_query}", self.url))
            .header("apikey", &self.anon)
            .bearer_auth(token)
            .send()
            .map_err(|e| format!("Could not reach Supabase: {e}"))?;
        let status = res.status();
        let text = res.text().map_err(|e| e.to_string())?;
        if !status.is_success() {
            return Err(body_error(status, text));
        }
        serde_json::from_str(&text).map_err(|e| format!("Bad response: {e}"))
    }
}

pub fn expiring_soon(expires_at: i64) -> bool {
    expires_at - now() < 120
}
