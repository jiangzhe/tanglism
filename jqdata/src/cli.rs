use crate::error::Error;
use crate::model::JqdataCommand;
use reqwest::header::{HeaderValue, CONTENT_TYPE};
use serde_json::json;
#[cfg(test)]
use mockito;

#[cfg(not(test))]
fn jqdata_url() -> &'static str {
    "https://dataapi.joinquant.com/apis"
}

#[cfg(test)]
fn jqdata_url() -> &'static str {
    &mockito::server_url()
}

pub struct JqdataClient {
    token: String,
}

/// retrieve token with given credential
fn get_token(mob: &str, pwd: &str, reuse: bool) -> Result<String, Error> {
    let method = if reuse {
        "get_current_token"
    } else {
        "get_token"
    };
    let token_req = json!({
        "method": method,
        "mob": mob,
        "pwd": pwd,
    });
    let client = reqwest::blocking::Client::new();
    let response = client
        .post(jqdata_url())
        .header(CONTENT_TYPE, HeaderValue::from_static("application/json"))
        .body(token_req.to_string())
        .send()?;
    let token: String = response.text()?;
    if token.starts_with("error") {
        return Err(Error::Server(token));
    }
    Ok(token)
}

impl JqdataClient {
    pub fn with_credential(mob: &str, pwd: &str) -> Result<Self, Error> {
        let token = get_token(mob, pwd, true)?;
        Ok(JqdataClient { token })
    }

    pub fn with_token(token: &str) -> Result<Self, Error> {
        Ok(JqdataClient {
            token: token.to_string(),
        })
    }

    pub fn execute<C: JqdataCommand>(&self, command: C) -> Result<C::Output, Error> {

        let req_body = command.request_body(&self.token)?;
        let client = reqwest::blocking::Client::new();
        let response = client
            .post(jqdata_url())
            .header(CONTENT_TYPE, HeaderValue::from_static("application/json"))
            .body(req_body)
            .send()?;
        let output = command.handle_response_body(response)?;
        Ok(output)
    }
}
