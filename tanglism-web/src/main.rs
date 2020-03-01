use std::{env, io};
use actix_web::{get, web, guard, middleware, App, HttpResponse, HttpServer, Responder, Result};
use actix_web::http::{header, Method, StatusCode};
use actix_files as fs;
use actix_session::{CookieSession, Session};

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    env::set_var("RUST_LOG", "actix_web=debug,actix_server=info");
    env_logger::init();

    HttpServer::new(|| {
        App::new()
            .wrap(CookieSession::signed(&[0; 32]).secure(false))
            .wrap(middleware::Logger::default())
            .service(index)
            .default_service(
                web::resource("")
                    .route(web::get().to(p404))
                    .route(web::route().guard(guard::Not(guard::Get())).to(HttpResponse::MethodNotAllowed)))
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await
}

#[get("/")]
async fn index() -> Result<fs::NamedFile> {
    Ok(fs::NamedFile::open("static/index.html")?)
}

async fn p404() -> Result<fs::NamedFile> {
    Ok(fs::NamedFile::open("static/404.html")?.set_status_code(StatusCode::NOT_FOUND))
}

