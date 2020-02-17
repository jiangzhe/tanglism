use jqdata::model::*;
use serde_json::json;

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
