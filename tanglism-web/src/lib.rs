mod routes;
mod handlers;
mod helpers;
mod errors;

use std::{env, io, net};
use actix_web::{middleware, HttpServer, App};
use actix_session::CookieSession;
use actix_cors::Cors;
use actix::prelude::*;
use crate::routes::routes;

#[derive(Message)]
#[rtype(result = "()")]
pub struct Message(pub String);

pub async fn server() -> io::Result<()> {
    env::set_var("RUST_LOG", "actix_web=debug,actix_server=info");
    env_logger::init();

    HttpServer::new(|| {
        App::new()
            .wrap(Cors::new().supports_credentials().finish())
            .wrap(CookieSession::signed(&[0; 32]).secure(false))
            .wrap(middleware::Logger::default())
            .configure(routes)
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await
}