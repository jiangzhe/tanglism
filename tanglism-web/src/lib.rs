mod errors;
mod handlers;
mod helpers;
mod routes;

use crate::routes::routes;
use actix::prelude::*;
use actix_cors::Cors;
use actix_session::CookieSession;
use actix_web::{middleware, App, HttpServer};
use std::{env, io, net};

pub use errors::Error;
pub type Result<T> = std::result::Result<T, Error>;

#[derive(Message)]
#[rtype(result = "()")]
pub struct Message(pub String);

pub async fn server(port: u32) -> std::io::Result<()> {
    env::set_var("RUST_LOG", "actix_web=debug,actix_server=info");
    env_logger::init();

    HttpServer::new(|| {
        App::new()
            .wrap(Cors::new().supports_credentials().finish())
            .wrap(CookieSession::signed(&[0; 32]).secure(false))
            .wrap(middleware::Logger::default())
            .configure(routes)
    })
    .bind(format!("127.0.0.1:{}", port))?
    .run()
    .await
}
