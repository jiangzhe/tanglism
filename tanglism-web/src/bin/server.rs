use dotenv::dotenv;
use std::env;
use structopt::StructOpt;
use tanglism_web::{server, Result};

#[tokio::main]
async fn main() -> Result<()> {
    // env::set_var("RUST_LOG", "actix_web=debug,actix_server=info,diesel=debug");
    env_logger::init();

    let opt = ServerOpt::from_args();
    dotenv().ok();

    let dburl = if let Some(url) = opt.dburl {
        url
    } else {
        env::var("DATABASE_URL").expect("DATABASE_URL should not be empty")
    };
    let jqaccount = if let Some(account) = opt.jqaccount {
        account
    } else {
        env::var("JQDATA_ACCOUNT").expect("JQDATA_ACCOUNT should not be empty")
    };
    server(&opt.host, opt.port, &dburl, &jqaccount).await?;
    Ok(())
}

#[derive(Debug, StructOpt)]
#[structopt(name = "tanglism-web", about = "command to run tanglism web server")]
pub struct ServerOpt {
    #[structopt(
        short,
        long,
        help = "specify server host, by default 127.0.0.1",
        default_value = "127.0.0.1"
    )]
    host: String,
    #[structopt(
        short,
        long,
        help = "specify server port to listen, by default 8080",
        default_value = "8080"
    )]
    port: u16,
    #[structopt(short, long, help = "specify dbfile to use")]
    dburl: Option<String>,
    #[structopt(short, long, help = "specify jqdata account to use")]
    jqaccount: Option<String>,
}
