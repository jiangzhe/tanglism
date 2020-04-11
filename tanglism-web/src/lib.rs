#[macro_use]
extern crate diesel;

mod errors;
mod handlers;
mod helpers;
pub mod models;
mod routes;
pub mod schema;

use crate::routes::routes;
use actix_cors::Cors;
use actix_session::CookieSession;
use actix_web::{middleware, App, HttpServer};
use diesel::pg::PgConnection;
use diesel::r2d2::{self, ConnectionManager};
use jqdata::JqdataClient;
use std::time::Duration;

pub use errors::{Error, ErrorKind};
pub type Result<T> = std::result::Result<T, Error>;
// use r2d2 to manage Postgres connections
type DbPool = r2d2::Pool<ConnectionManager<PgConnection>>;

pub async fn server(host: &str, port: u32, dburl: &str, jqaccount: &str) -> Result<()> {
    let manager = ConnectionManager::<PgConnection>::new(dburl);
    let pool = r2d2::Pool::builder()
        .connection_timeout(Duration::from_secs(3))
        .build(manager)
        .expect("Failed to create db connection pool");
    let (jqmob, jqpwd) = parse_jqaccount(jqaccount)?;
    let jq = JqdataClient::with_credential(jqmob, jqpwd).await?;

    let svr = HttpServer::new(move || {
        App::new()
            .data(pool.clone())
            .data(jq.clone())
            .wrap(Cors::new().supports_credentials().finish())
            .wrap(CookieSession::signed(&[0; 32]).secure(false))
            .wrap(middleware::Logger::default())
            .configure(routes)
    })
    .bind(format!("{}:{}", host, port))?;
    svr.run().await?;
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
