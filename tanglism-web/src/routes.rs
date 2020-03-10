use crate::handlers::health::get_health;
use actix_files::Files;
use actix_web::web;

pub fn routes(cfg: &mut web::ServiceConfig) {
    cfg.route("/health", web::get().to(get_health)).service(
        web::scope("").default_service(
            Files::new("", "./static")
                .index_file("index.html")
                .use_last_modified(true),
        ),
    );
}
