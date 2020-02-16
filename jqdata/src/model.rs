use crate::error::Error;
//use serde::{Serialize, Deserialize};
use serde_derive::*;
use serde_json::json;
use reqwest::blocking::Response;

pub trait JqdataCommand {
    type Output;
    // generate request body, with given token
    fn request_body(&self, token: &str) -> Result<String, Error>;

    // parse response body into proper data structure
    fn handle_response_body(&self, response: Response) -> Result<Self::Output, Error>;
}

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


#[derive(Debug, Serialize, Deserialize)]
pub struct GetAllSecurities {
    pub code: SecurityKind,
    pub date: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
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

impl JqdataCommand for GetAllSecurities {
    type Output = Vec<Security>;

    fn request_body(&self, token: &str) -> Result<String, Error> {
        let json = serde_json::to_string(&json!({
            "method": "get_all_securities",
            "token": token,
            "code": self.code,
            "date": self.date,
        }))?;
        Ok(json)
    }

    fn handle_response_body(&self, mut response: Response) -> Result<Self::Output, Error> {
        let mut reader = csv::Reader::from_reader(&mut response);
        let mut rs = Vec::new();
        for r in reader.deserialize() {
            let s: Security = r?;
            rs.push(s);
        }
        Ok(rs)
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
        let gas = GetAllSecurities{code: SecurityKind::Stock, date: Some(String::from("2020-02-16"))};
        assert_eq!(serde_json::to_string(&json!({
            "method": "get_all_securities",
            "token": "abc",
            "code": "stock",
            "date": "2020-02-16",
        })).unwrap(), gas.request_body("abc").unwrap());
        
        
    }

    fn assert_serde_security_kind(s: &str, k: &SecurityKind) {
        let str_repr = serde_json::to_string(s).unwrap();
        assert_eq!(str_repr, serde_json::to_string(k).unwrap());
        assert_eq!(k, &serde_json::from_str::<SecurityKind>(&str_repr).unwrap());
    }
}