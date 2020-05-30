#![forbid(unsafe_code)]

#[macro_use]
extern crate diesel;

mod errors;
mod handlers;
pub mod models;
mod routes;
pub mod schema;
mod ws;

use chrono::NaiveDateTime;
use diesel::pg::PgConnection;
use diesel::r2d2::{self, ConnectionManager};
use jqdata::JqdataClient;
use serde_derive::*;
use std::time::Duration;
use warp::http::Uri;
use warp::Filter;

pub use errors::{Error, ErrorKind};
pub type Result<T> = std::result::Result<T, Error>;

// use r2d2 to manage Postgres connections
type DbPool = r2d2::Pool<ConnectionManager<PgConnection>>;

// 股票基础配置
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct BasicCfg {
    tick: String,
    code: String,
    start_ts: NaiveDateTime,
    end_ts: NaiveDateTime,
}

pub async fn server(host: &str, port: u16, dburl: &str, jqaccount: &str) -> Result<()> {
    let host: std::net::IpAddr = host.parse().expect("host must be string of IPv4");
    let manager = ConnectionManager::<PgConnection>::new(dburl);
    let pool = r2d2::Pool::builder()
        .connection_timeout(Duration::from_secs(3))
        .build(manager)
        .expect("Failed to create db connection pool");
    let (jqmob, jqpwd) = parse_jqaccount(jqaccount)?;
    let jq = JqdataClient::with_credential(jqmob, jqpwd).await?;

    // 主页重定向
    let index = warp::get()
        .and(warp::path::end())
        .map(|| warp::redirect(Uri::from_static("/static/index.html")));
    // websocket
    let ws_filter = ws::ws_filter(jq, pool.clone());

    // API路由
    let apis = routes::api_route(pool);

    // 静态资源文件
    let files = warp::get()
        .and(warp::path("static"))
        .and(warp::fs::dir("./static/"));

    let routes = index.or(ws_filter).or(apis).or(files);
    warp::serve(routes).run((host, port)).await;
    Ok(())
}

fn parse_jqaccount(account: &str) -> Result<(String, String)> {
    let splits: Vec<&str> = account.split('/').collect();
    if splits.len() != 2 {
        return Err(Error::Custom(
            ErrorKind::InternalServerError,
            "invalid jqdata account".into(),
        ));
    }
    Ok((splits[0].to_owned(), splits[1].to_owned()))
}
