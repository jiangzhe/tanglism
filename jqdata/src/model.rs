use crate::error::Error;
use reqwest::blocking::Response;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::str::FromStr;

/// RequestCommand
/// 
/// define how to generate request body
pub trait RequestCommand {
    // generate request body, with given token
    fn request_body(&self, token: &str) -> Result<String, Error>;
}

/// JqdataCommand
///
/// defines how to handle response body
pub trait JqdataCommand: RequestCommand {
    type Output;
    // response is consumed, and the parsed output is returned
    fn response_body(&self, response: Response) -> Result<Self::Output, Error>;
}

// csv consuming function
fn consume_csv<T>(response: &mut Response) -> Result<Vec<T>, Error>
where
    for<'de> T: Deserialize<'de>,
{
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .from_reader(response);
    let mut rs = Vec::new();
    for r in reader.deserialize() {
        let s: T = r?;
        rs.push(s);
    }
    Ok(rs)
}

#[derive(Debug, Serialize, Deserialize)]
pub enum TimeUnit {
    #[serde(rename = "1m")]
    U1m,
    #[serde(rename = "5m")]
    U5m,
    #[serde(rename = "15m")]
    U15m,
    #[serde(rename = "30m")]
    U30m,
    #[serde(rename = "60m")]
    U60m,
    #[serde(rename = "120m")]
    U120m,
    #[serde(rename = "1d")]
    U1d,
    #[serde(rename = "1w")]
    U1w,
    // 1M is not included for simplicity
    // U1M,
}

/// enable parse string to time unit
impl FromStr for TimeUnit {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "1m" => Ok(TimeUnit::U1m),
            "5m" => Ok(TimeUnit::U5m),
            "15m" => Ok(TimeUnit::U15m),
            "30m" => Ok(TimeUnit::U30m),
            "60m" => Ok(TimeUnit::U60m),
            "120m" => Ok(TimeUnit::U120m),
            "1d" => Ok(TimeUnit::U1d),
            "1w" => Ok(TimeUnit::U1w),
            // "1M" => Ok(TimeUnit::U1M),
            _ => Err(Error::Client(format!("invalid time unit: {}", s))),
        }
    }
}

/// kind of security
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SecurityKind {
    Stock,
    Fund,
    Index,
    Futures,
    #[serde(rename = "etf")]
    ETF,
    #[serde(rename = "lof")]
    LOF,
    #[serde(rename = "fja")]
    FJA,
    #[serde(rename = "fjb")]
    FJB,
    #[serde(rename = "QDII_fund")]
    QDIIFund,
    OpenFund,
    BondFund,
    StockFund,
    MoneyMarketFund,
    MixtureFund,
    Options,
}

/// security model
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct Security {
    pub code: String,
    pub display_name: String,
    pub name: String,
    pub start_date: String,
    pub end_date: String,
    #[serde(rename = "type")]
    pub kind: SecurityKind,
    pub parent: Option<String>,
}

/// all requests are defined below

/// 获取平台支持的所有股票、基金、指数、期货信息
#[derive(Debug, Serialize, Deserialize)]
pub struct GetAllSecurities {
    pub code: SecurityKind,
    pub date: Option<String>,
}

impl RequestCommand for GetAllSecurities {
    fn request_body(&self, token: &str) -> Result<String, Error> {
        let json = serde_json::to_string(&json!({
            "method": "get_all_securities",
            "token": token,
            "code": self.code,
            "date": self.date,
        }))?;
        Ok(json)
    }
}

impl JqdataCommand for GetAllSecurities {
    type Output = Vec<Security>;
    fn response_body(&self, mut response: Response) -> Result<Vec<Security>, Error> {
        consume_csv(&mut response)
    }
}

/// 获取股票/基金/指数的信息
pub struct GetSecurityInfo {
    pub code: String,
}

impl RequestCommand for GetSecurityInfo {
    fn request_body(&self, token: &str) -> Result<String, Error> {
        let json = serde_json::to_string(&json!({
            "method": "get_security_info",
            "token": token,
            "code": self.code,
        }))?;
        Ok(json)
    }
}

impl JqdataCommand for GetSecurityInfo {
    type Output = Vec<Security>;
    fn response_body(&self, mut response: Response) -> Result<Vec<Security>, Error> {
        consume_csv(&mut response)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_security_kind_rename() {
        assert_serde_security_kind("stock", &SecurityKind::Stock);
        assert_serde_security_kind("fund", &SecurityKind::Fund);
        assert_serde_security_kind("index", &SecurityKind::Index);
        assert_serde_security_kind("futures", &SecurityKind::Futures);
        assert_serde_security_kind("etf", &SecurityKind::ETF);
        assert_serde_security_kind("lof", &SecurityKind::LOF);
        assert_serde_security_kind("fja", &SecurityKind::FJA);
        assert_serde_security_kind("fjb", &SecurityKind::FJB);
        assert_serde_security_kind("QDII_fund", &SecurityKind::QDIIFund);
        assert_serde_security_kind("open_fund", &SecurityKind::OpenFund);
        assert_serde_security_kind("bond_fund", &SecurityKind::BondFund);
        assert_serde_security_kind("stock_fund", &SecurityKind::StockFund);
        assert_serde_security_kind("money_market_fund", &SecurityKind::MoneyMarketFund);
        assert_serde_security_kind("mixture_fund", &SecurityKind::MixtureFund);
        assert_serde_security_kind("options", &SecurityKind::Options);
    }

    #[test]
    fn test_get_all_securities() {
        let gas = GetAllSecurities {
            code: SecurityKind::Stock,
            date: Some(String::from("2020-02-16")),
        };
        assert_eq!(
            serde_json::to_string(&json!({
                "method": "get_all_securities",
                "token": "abc",
                "code": "stock",
                "date": "2020-02-16",
            }))
            .unwrap(),
            gas.request_body("abc").unwrap()
        );
    }

    fn assert_serde_security_kind(s: &str, k: &SecurityKind) {
        let str_repr = serde_json::to_string(s).unwrap();
        assert_eq!(str_repr, serde_json::to_string(k).unwrap());
        assert_eq!(k, &serde_json::from_str::<SecurityKind>(&str_repr).unwrap());
    }
}
