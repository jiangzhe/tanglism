use crate::handlers::stock_prices::ticks;
use crate::handlers::{choice, metrics, stocks};
use crate::DbPool;
use bigdecimal::BigDecimal;
use chrono::{Local, NaiveDate};
use serde_derive::*;
use std::convert::Infallible;
use tanglism_utils::{LocalTradingTimestamps, TradingDates};
use warp::Filter;

/// API入口
pub fn api_route(
    db: DbPool,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    api_get_health()
        .or(api_search_keyword_stocks(db.clone()))
        .or(api_list_prioritized_stocks(db.clone()))
        .or(api_list_choices(db))
}

/// REST API: 健康检查
fn api_get_health() -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path!("api" / "health").and(warp::get()).map(|| {
        warp::reply::json(&HealthResponse {
            status: "ok".into(),
            version: env!("CARGO_PKG_VERSION").into(),
        })
    })
}

/// REST API: 根据关键字搜索股票
pub fn api_search_keyword_stocks(
    db: DbPool,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path!("api" / "keyword-stocks")
        .and(warp::query::<SearchKeywordStocksParam>())
        .and(with_db(db))
        .and_then(search_keyword_stocks)
}

/// REST API: 查询重点股票
pub fn api_list_prioritized_stocks(
    db: DbPool,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path!("api" / "prioritized-stocks")
        .and(warp::query::<ListPrioritizedStocksParam>())
        .and(with_db(db))
        .and_then(list_prioritized_stocks)
}

/// REST API: 查询机会股票
pub fn api_list_choices(
    db: DbPool,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path!("api" / "choices")
        .and(warp::query::<ListChoicesParam>())
        .and(with_db(db))
        .and_then(list_choices)
}

/// 注入db的公共过滤器
fn with_db(db: DbPool) -> impl Filter<Extract = (DbPool,), Error = Infallible> + Clone {
    warp::any().map(move || db.clone())
}

async fn search_keyword_stocks(
    param: SearchKeywordStocksParam,
    db: DbPool,
) -> Result<impl warp::Reply, warp::Rejection> {
    match stocks::search_keyword_stocks(db, param.keyword).await {
        Ok(data) => Ok(warp::reply::json(&data)),
        Err(err) => Err(warp::reject::custom(err)),
    }
}

async fn list_prioritized_stocks(
    param: ListPrioritizedStocksParam,
    db: DbPool,
) -> Result<impl warp::Reply, warp::Rejection> {
    match param.atrp_days {
        Some(atrp_days) => {
            let rs = stocks::search_prioritized_stocks(db.clone())
                .await
                .map_err(warp::reject::custom)?;
            let codes = rs.iter().map(|s| s.code.clone()).collect();
            let tick = "1d".to_owned();
            let today = Local::today().naive_local();
            let tts = LocalTradingTimestamps::new("1d").unwrap();
            let end_dt = if tts.contains_day(today) {
                today
            } else {
                tts.prev_day(today).unwrap()
            };
            let mut start_dt = end_dt;
            // 这里多取1天，因为最早一天无法计算ATR，会被舍去
            for _ in 0..atrp_days {
                start_dt = tts.prev_day(start_dt).unwrap();
            }
            let data = ticks::query_db_multiple_prices(db, tick, codes, start_dt, end_dt)
                .await
                .map_err(warp::reject::custom)?;
            let atrp_stats = metrics::multi_atrp_stats(&data);
            let rst: Vec<_> = rs
                .into_iter()
                .map(|s| {
                    if let Some(stats) = atrp_stats.get(&s.code) {
                        PrioritizedStockAtrpStats {
                            code: s.code,
                            display_name: s.display_name,
                            msci: s.msci,
                            hs300: s.hs300,
                            atrp_days: stats.data.len(),
                            atrp_max: Some(stats.max.with_prec(6)),
                            atrp_min: Some(stats.min.with_prec(6)),
                            atrp_avg: Some(stats.avg.with_prec(6)),
                        }
                    } else {
                        PrioritizedStockAtrpStats {
                            code: s.code,
                            display_name: s.display_name,
                            msci: s.msci,
                            hs300: s.hs300,
                            atrp_days: 0,
                            atrp_max: None,
                            atrp_min: None,
                            atrp_avg: None,
                        }
                    }
                })
                .collect();
            Ok(warp::reply::json(&rst))
        }
        None => {
            let rs = stocks::search_prioritized_stocks(db)
                .await
                .map_err(warp::reject::custom)?;
            let rst: Vec<_> = rs
                .into_iter()
                .map(|r| PrioritizedStock {
                    code: r.code,
                    display_name: r.display_name,
                })
                .collect();
            Ok(warp::reply::json(&rst))
        }
    }
}

async fn list_choices(
    param: ListChoicesParam,
    db: DbPool,
) -> Result<impl warp::Reply, warp::Rejection> {
    match choice::list_choices(db, param.days.unwrap_or(22), param.limit.unwrap_or(10)).await {
        Ok(data) => Ok(warp::reply::json(&data)),
        Err(err) => Err(warp::reject::custom(err)),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
}

/// 股票关键字搜索参数
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchKeywordStocksParam {
    pub keyword: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetStockPricesParam {
    pub start_dt: NaiveDate,
    pub end_dt: NaiveDate,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListPrioritizedStocksParam {
    pub atrp_days: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrioritizedStock {
    pub code: String,
    pub display_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrioritizedStockAtrpStats {
    pub code: String,
    pub display_name: String,
    pub msci: bool,
    pub hs300: bool,
    pub atrp_days: usize,
    pub atrp_max: Option<BigDecimal>,
    pub atrp_min: Option<BigDecimal>,
    pub atrp_avg: Option<BigDecimal>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListChoicesParam {
    pub days: Option<usize>,
    pub limit: Option<usize>,
}
