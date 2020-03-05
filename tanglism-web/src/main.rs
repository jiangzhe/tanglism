use rusqlite::{Connection, OpenFlags};
use structopt::StructOpt;
use tanglism_web::{server, Error, Result};

#[actix_rt::main]
async fn main() -> Result<()> {
    let opt = Opt::from_args();
    let mut conn = Connection::open_with_flags(opt.dbfile, OpenFlags::SQLITE_OPEN_READ_ONLY)?;

    let _stopped = server(opt.port).await?;
    Ok(())
}

#[derive(Debug, StructOpt)]
#[structopt(name = "tanglism-web", about = "command to run tanglism web server")]
pub struct Opt {
    #[structopt(
        short,
        long,
        help = "specify server port to listen, by default 8080",
        default_value = "8080"
    )]
    port: u32,
    #[structopt(
        short,
        long,
        help = "specify dbfile to use",
        default_value = "./jqdata.db"
    )]
    dbfile: String,
}
