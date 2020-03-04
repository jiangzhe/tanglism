use tanglism_web::server;

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    server().await
}
