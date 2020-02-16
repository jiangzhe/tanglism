use reqwest::header::{HeaderValue, CONTENT_TYPE};
use serde_json::json;
use crate::error::Error;

const JQDATA_URL: &str = "https://dataapi.joinquant.com/apis";

pub struct JqdataClient {
    token: String,
}

impl JqdataClient {
    pub fn with_credential(mob: &str, pwd: &str) -> Result<Self, Error> {
        let token_req = json!({
        "method": "get_token",
        "mob": mob,
        "pwd": pwd,
        });
        let client = reqwest::blocking::Client::new();
        let response = client.post(JQDATA_URL)
            .header(CONTENT_TYPE, HeaderValue::from_static("application/json"))
            .body(token_req.to_string())
            .send()?;
        let text: String = response.text()?;
        if text.starts_with("error") {
            return Err(Error::Server(text));
        }
        let cli = JqdataClient{
            token: text,
        };
        Ok(cli)
    }

    pub fn with_token(token: &str) -> Result<Self, Error> {
        Ok(JqdataClient{token: token.to_string()})
    }

    pub fn get_price_period(&self, code: &str, unit: &str, date: &str, end_date: &str, fq_ref_date: Option<&str>) {
        unimplemented!()
    }
}