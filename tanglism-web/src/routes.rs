use crate::handlers::health::api_get_health;
use crate::handlers::stock_prices::{api_get_stock_daily_prices, api_get_stock_tick_prices};
use crate::handlers::stocks::api_search_keyword_stocks;
use crate::handlers::trade_days::api_get_trade_days;
use crate::handlers::tanglism::{api_get_tanglism_partings, api_get_tanglism_strokes, api_get_tanglism_segments};
use actix_files::Files;
use actix_web::web;

pub fn routes(cfg: &mut web::ServiceConfig) {
    cfg.route("/health", web::get().to(api_get_health))
        .service(
            web::scope("/api/v1")
                .service(api_get_trade_days)
                .service(api_search_keyword_stocks)
                .service(api_get_stock_daily_prices)
                .service(api_get_stock_tick_prices)
                .service(api_get_tanglism_partings)
                .service(api_get_tanglism_strokes)
                .service(api_get_tanglism_segments),
        )
        .service(
            web::scope("").default_service(
                Files::new("", "./static")
                    .index_file("index.html")
                    .use_last_modified(true),
            ),
        );
}
