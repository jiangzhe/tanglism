use actix_web::web;
use actix_files::Files;
use crate::handlers::health::get_health;


pub fn routes(cfg: &mut web::ServiceConfig) {
    cfg
        .route("/health", web::get().to(get_health))
        .service(
            web::scope("").default_service(
                Files::new("", "./static")
                    .index_file("index.html")
                    .use_last_modified(true),
            ),
        );
}