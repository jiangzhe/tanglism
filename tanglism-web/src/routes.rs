use crate::handlers::stocks;
use crate::DbPool;
use serde_derive::*;
use std::convert::Infallible;
use warp::Filter;

/// API入口
pub fn api_route(
    db: DbPool,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    api_get_health().or(api_search_keyword_stocks(db))
}

/// 注入db的公共过滤器
fn with_db(db: DbPool) -> impl Filter<Extract = (DbPool,), Error = Infallible> + Clone {
    warp::any().map(move || db.clone())
}

/// 股票关键字搜索参数
#[derive(Debug, Serialize, Deserialize)]
pub struct SearchKeywordStocksParam {
    pub keyword: String,
}

fn api_search_keyword_stocks(
    db: DbPool,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path!("api" / "keyword-stocks")
        .and(warp::query::<SearchKeywordStocksParam>())
        .and(with_db(db))
        .and_then(search_keyword_stocks)
}

pub async fn search_keyword_stocks(
    param: SearchKeywordStocksParam,
    db: DbPool,
) -> Result<impl warp::Reply, warp::Rejection> {
    match stocks::search_keyword_stocks(db, param.keyword).await {
        Ok(data) => Ok(warp::reply::json(&data)),
        Err(err) => Err(warp::reject::custom(err)),
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
}

fn api_get_health() -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path!("api" / "health").and(warp::get()).map(|| {
        warp::reply::json(&HealthResponse {
            status: "ok".into(),
            version: env!("CARGO_PKG_VERSION").into(),
        })
    })
}
