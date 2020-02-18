use crate::error::Error;
use jqdata_derive::*;
#[allow(unused_imports)]
use serde::{Deserialize, Serialize};
#[allow(unused_imports)]
use serde_derive::*;
use std::io::{Read, BufRead};
use std::str::FromStr;

/// Request
///
/// define how to generate request body
pub trait Request {
    // generate request body, with given token
    fn request(&self, token: &str) -> Result<String, Error>;
}

/// Response
///
/// defines how to handle response body
pub trait Response {
    type Output;
    // response is consumed, and the parsed output is returned
    fn response(&self, response: reqwest::blocking::Response) -> Result<Self::Output, Error>;
}

// csv consuming function, used by derive macro
#[allow(dead_code)]
pub(crate) fn consume_csv<T>(response: &mut reqwest::blocking::Response) -> Result<Vec<T>, Error>
where
    for<'de> T: Deserialize<'de>,
{
    let mut reader = csv::ReaderBuilder::new()
        // .has_headers(true)
        .from_reader(response);
    // consume the first row as header
    let header_cols: Vec<&str> = reader.headers()?.into_iter().collect();
    if header_cols.is_empty() {
        return Err(Error::Server("empty response body returned".to_owned()));
    }
    let first_col = header_cols.first().cloned().unwrap();
    if first_col.starts_with("error") {
        return Err(Error::Server(first_col.to_owned()));
    }
    let mut rs = Vec::new();
    for r in reader.deserialize() {
        let s: T = r?;
        rs.push(s);
    }
    Ok(rs)
}

// line consuming function, used by derive macro
#[allow(dead_code)]
pub(crate) fn consume_line(
    response: &mut reqwest::blocking::Response,
) -> Result<Vec<String>, Error> {
    let reader = std::io::BufReader::new(response);
    let mut rs = Vec::new();
    for line in reader.lines() {
        rs.push(line?);
    }
    Ok(rs)
}

// json consuming function, used by derive macro
#[allow(dead_code)]
pub(crate) fn consume_json<T>(response: &mut reqwest::blocking::Response) -> Result<T, Error>
where 
    for<'de> T: Deserialize<'de>,
{
    let result = serde_json::from_reader(response)?;
    Ok(result)
}

// 时间周期
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

/// 证券类型
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

/// 证券信息
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct Security {
    pub code: String,
    pub display_name: String,
    pub name: String,
    pub start_date: String,
    pub end_date: String,
    #[serde(rename = "type")]
    pub kind: SecurityKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
}

/// all requests are defined below

/// 获取平台支持的所有股票、基金、指数、期货信息
#[derive(Debug, Serialize, Deserialize, Request, Response)]
#[request(get_all_securities)]
#[response(format = "csv", type = "Security")]
pub struct GetAllSecurities {
    pub code: SecurityKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
}

/// 获取股票/基金/指数的信息
#[derive(Debug, Serialize, Deserialize, Request, Response)]
#[request(get_security_info)]
#[response(format = "csv", type = "Security")]
pub struct GetSecurityInfo {
    pub code: String,
}

/// 获取一个指数给定日期在平台可交易的成分股列表
#[derive(Debug, Serialize, Deserialize, Request, Response)]
#[request(get_index_stocks)]
#[response(format = "line")]
pub struct GetIndexStocks {
    pub code: String,
    pub date: String,
}

/// 获取指定日期上交所、深交所披露的的可融资标的列表
/// 查询日期，默认为前一交易日
#[derive(Debug, Serialize, Deserialize, Request, Response)]
#[request(get_margincash_stocks)]
#[response(format = "line")]
pub struct GetMargincashStocks {
    pub date: Option<String>,
}

/// 获取指定日期区间内的限售解禁数据
#[derive(Debug, Serialize, Deserialize, Request, Response)]
#[request(get_locked_shares)]
#[response(format = "csv", type = "LockedShare")]
pub struct GetLockedShares {
    pub code: String,
    pub date: String,
    pub end_date: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LockedShare {
    pub day: String,
    pub code: String,
    pub num: f64,
    pub rate1: f64,
    pub rate2: f64,
}

/// 获取指数成份股给定日期的权重数据，每月更新一次
/// code: 代表指数的标准形式代码， 形式：指数代码.交易所代码，例如"000001.XSHG"。
/// date: 查询权重信息的日期，形式："%Y-%m-%d"，例如"2018-05-03"；
#[derive(Debug, Serialize, Deserialize, Request, Response)]
#[request(get_index_weights)]
#[response(format = "csv", type = "IndexWeight")]
pub struct GetIndexWeights {
    pub code: String,
    pub date: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct IndexWeight {
    pub code: String,
    pub display_name: String,
    pub date: String,
    pub weight: f64,
}

/// 按照行业分类获取行业列表
/// code：行业代码
/// sw_l1: 申万一级行业
/// sw_l2: 申万二级行业
/// sw_l3: 申万三级行业
/// jq_l1: 聚宽一级行业
/// jq_l2: 聚宽二级行业
/// zjw: 证监会行业
#[derive(Debug, Serialize, Deserialize, Request, Response)]
#[request(get_industries)]
#[response(format = "csv", type = "IndustryIndex")]
pub struct GetIndustries {
    pub code: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct IndustryIndex {
    pub index: String,
    pub name: String,
    pub start_date: String,
}

/// 查询股票所属行业
/// 参数：
/// code：证券代码
/// date：查询的日期
#[derive(Debug, Serialize, Deserialize, Request, Response)]
#[request(get_industry)]
#[response(format = "csv", type = "Industry")]
pub struct GetIndustry {
    pub code: String,
    pub date: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Industry {
    pub industry: String,
    pub industry_code: String,
    pub industry_name: String,
}

/// 获取在给定日期一个行业的所有股票
/// 参数：
/// code: 行业编码
/// date: 查询日期
#[derive(Debug, Serialize, Deserialize, Request, Response)]
#[request(get_industry_stocks)]
#[response(format = "line")]
pub struct GetIndustryStocks {
    pub code: String,
    pub date: String,
}

/// 获取在给定日期一个概念板块的所有股票
/// 参数：
/// code: 概念板块编码
/// date: 查询日期,
#[derive(Debug, Serialize, Deserialize, Request, Response)]
#[request(get_concepts)]
#[response(format = "csv", type = "Concept")]
pub struct GetConcepts {}

#[derive(Debug, Serialize, Deserialize)]
pub struct Concept {
    pub code: String,
    pub name: String,
    pub start_date: String,
}

/// 获取在给定日期一个概念板块的所有股票
/// 参数：
/// code: 概念板块编码
/// date: 查询日期,
#[derive(Debug, Serialize, Deserialize, Request, Response)]
#[request(get_concept_stocks)]
#[response(format = "line")]
pub struct GetConceptStocks {
    pub code: String,
    pub date: String,
}

/// 获取指定日期范围内的所有交易日
/// 参数：
/// date: 开始日期
/// end_date: 结束日期
#[derive(Debug, Serialize, Deserialize, Request, Response)]
#[request(get_trade_days)]
#[response(format = "line")]
pub struct GetTradeDays {
    pub date: String,
    pub end_date: String,
}

/// 获取所有交易日
#[derive(Debug, Serialize, Deserialize, Request, Response)]
#[request(get_all_trade_days)]
#[response(format = "line")]
pub struct GetAllTradeDays {}

/// 获取一只股票在一个时间段内的融资融券信息
/// 参数：
/// code: 股票代码
/// date: 开始日期
/// end_date: 结束日期
/// 返回：
/// date: 日期
/// sec_code: 股票代码
/// fin_value: 融资余额(元）
/// fin_buy_value: 融资买入额（元）
/// fin_refund_value: 融资偿还额（元）
/// sec_value: 融券余量（股）
/// sec_sell_value: 融券卖出量（股）
/// sec_refund_value: 融券偿还量（股）
/// fin_sec_value: 融资融券余额（元）
#[derive(Debug, Serialize, Deserialize, Request, Response)]
#[request(get_mtss)]
#[response(format = "csv", type = "Mtss")]
pub struct GetMtss {
    pub code: String,
    pub date: String,
    pub end_date: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Mtss {
    pub date: String,
    pub sec_code: String,
    pub fin_value: f64,
    pub fin_refund_value: f64,
    pub sec_value: f64,
    pub sec_sell_value: f64,
    pub sec_refund_value: f64,
    pub fin_sec_value: f64,
}

/// 获取一只股票在一个时间段内的资金流向数据，仅包含股票数据，不可用于获取期货数据
/// 参数：
/// code: 股票代码
/// date: 开始日期
/// end_date: 结束日期
/// 返回：
/// date: 日期
/// sec_code: 股票代码
/// change_pct: 涨跌幅(%)
/// net_amount_main: 主力净额(万): 主力净额 = 超大单净额 + 大单净额
/// net_pct_main: 主力净占比(%): 主力净占比 = 主力净额 / 成交额
/// net_amount_xl: 超大单净额(万): 超大单：大于等于50万股或者100万元的成交单
/// net_pct_xl: 超大单净占比(%): 超大单净占比 = 超大单净额 / 成交额
/// net_amount_l: 大单净额(万): 大单：大于等于10万股或者20万元且小于50万股或者100万元的成交单
/// net_pct_l: 大单净占比(%): 大单净占比 = 大单净额 / 成交额
/// net_amount_m: 中单净额(万): 中单：大于等于2万股或者4万元且小于10万股或者20万元的成交单
/// net_pct_m: 中单净占比(%): 中单净占比 = 中单净额 / 成交额
/// net_amount_s: 小单净额(万): 小单：小于2万股或者4万元的成交单
/// net_pct_s: 小单净占比(%): 小单净占比 = 小单净额 / 成交额
#[derive(Debug, Serialize, Deserialize, Request, Response)]
#[request(get_money_flow)]
#[response(format = "csv", type = "MoneyFlow")]
pub struct GetMoneyFlow {
    pub code: String,
    pub date: String,
    pub end_date: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MoneyFlow {
    pub date: String,
    pub sec_code: String,
    pub change_pct: f64,
    pub net_amount_main: f64,
    pub net_pct_main: f64,
    pub net_amount_xl: f64,
    pub net_pct_xl: f64,
    pub net_amount_l: f64,
    pub net_pct_l: f64,
    pub net_amount_m: f64,
    pub net_pct_m: f64,
    pub net_amount_s: f64,
    pub net_pct_s: f64,
}

/// 获取指定日期区间内的龙虎榜数据
/// 参数：
/// code: 股票代码
/// date: 开始日期
/// end_date: 结束日期
/// 返回：
/// code: 股票代码
/// day: 日期
/// direction: ALL 表示『汇总』，SELL 表示『卖』，BUY 表示『买』
/// abnormal_code: 异常波动类型
/// abnormal_name: 异常波动名称
/// sales_depart_name: 营业部名称
/// rank: 0 表示汇总， 1~5 表示买一到买五， 6~10 表示卖一到卖五
/// buy_value: 买入金额
/// buy_rate: 买入金额占比(买入金额/市场总成交额)
/// sell_value: 卖出金额
/// sell_rate: 卖出金额占比(卖出金额/市场总成交额)
/// net_value: 净额(买入金额 - 卖出金额)
/// amount: 市场总成交额
#[derive(Debug, Serialize, Deserialize, Request, Response)]
#[request(get_billboard_list)]
#[response(format = "csv", type = "BillboardStock")]
pub struct GetBillboardList {
    pub code: String,
    pub date: String,
    pub end_date: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BillboardStock {
    pub code: String,
    pub day: String,
    pub direction: String,
    pub rank: i32,
    pub abnormal_code: String,
    pub abnormal_name: String,
    pub sales_depart_name: String,
    pub buy_value: f64,
    pub buy_rate: f64,
    pub sell_value: f64,
    pub sell_rate: f64,
    pub total_value: f64,
    pub net_value: f64,
    pub amount: f64,
}


/// 获取某期货品种在指定日期下的可交易合约标的列表
/// 参数：
/// code: 期货合约品种，如 AG (白银)
/// date: 指定日期
#[derive(Debug, Serialize, Deserialize, Request, Response)]
#[request(get_future_contracts)]
#[response(format = "line")]
pub struct GetFutureContracts {
    pub code: String,
    pub date: String,
}


/// 获取主力合约对应的标的
/// 参数：
/// code: 期货合约品种，如 AG (白银)
/// date: 指定日期参数，获取历史上该日期的主力期货合约
#[derive(Debug, Serialize, Deserialize, Request, Response)]
#[request(get_dominant_future)]
#[response(format = "line")]
pub struct GetDominantFuture {
    pub code: String,
    pub date: String,
}

/// 获取单个基金的基本信息
/// 参数：
/// code: 基金代码
/// date: 查询日期， 默认日期是今天。
/// 返回：
/// fund_name: 基金全称
/// fund_type: 基金类型
/// fund_establishment_day: 基金成立日
/// fund_manager: 基金管理人及基本信息
/// fund_management_fee: 基金管理费
/// fund_custodian_fee: 基金托管费
/// fund_status: 基金申购赎回状态
/// fund_size: 基金规模（季度）
/// fund_share: 基金份额（季度）
/// fund_asset_allocation_proportion: 基金资产配置比例（季度）
/// heavy_hold_stocks: 基金重仓股（季度）
/// heavy_hold_stocks_proportion: 基金重仓股占基金资产净值比例（季度）
/// heavy_hold_bond: 基金重仓债券（季度）
/// heavy_hold_bond_proportion: 基金重仓债券占基金资产净值比例（季度）
#[derive(Debug, Serialize, Deserialize, Request, Response)]
#[request(get_fund_info)]
#[response(format = "json", type = "FundInfo")]
pub struct GetFundInfo {
    pub code: String,
    pub date: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FundInfo {
    pub fund_name: String,
    pub fund_type: String,
    pub fund_establishment_day: String,
    pub fund_manager: String,
    pub fund_management_fee: String,
    pub fund_custodian_fee: String,
    pub fund_status: String,
    pub fund_size: String,
    pub fund_share: f64,
    pub fund_asset_allocation_proportion: String,
    pub heavy_hold_stocks: Vec<String>,
    pub heavy_hold_stocks_proportion: f64,
    pub heavy_hold_bond: Vec<String>,
    pub heavy_hold_bond_proportion: f64,
}


#[cfg(test)]
mod tests {
    use super::*;
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
            gas.request("abc").unwrap()
        );
    }

    fn assert_serde_security_kind(s: &str, k: &SecurityKind) {
        let str_repr = serde_json::to_string(s).unwrap();
        assert_eq!(str_repr, serde_json::to_string(k).unwrap());
        assert_eq!(k, &serde_json::from_str::<SecurityKind>(&str_repr).unwrap());
    }

}
