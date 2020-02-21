use rusqlite::{Connection, OpenFlags};
use jqdata::JqdataClient;
use structopt::StructOpt;
use structopt_derive::*;
use jqdata_shell::{Error, DatabasePopulator};

fn main() -> std::result::Result<(), Error> {
    let opt = Opt::from_args();
    let cli = JqdataClient::with_credential(&opt.mob, &opt.pwd)?;
    let conn = Connection::open_with_flags(opt.file.unwrap_or("./jqdata.db".to_owned()),
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE)?;
    let populator = DatabasePopulator::new(&conn, &cli);

    match opt.cmd {
        Command::Insert { table, since } => {
            match &table[..] {
                "trade_days" => {
                    populator.populate_trade_days(since)?;
                }
                _ => return Err(Error(format!("unknown table {}", table)))
            }
        }
    }
    Ok(())
}

#[derive(Debug, StructOpt)]
#[structopt(name = "jqdata-shell", about = "shell to run jqdata commands")]
struct Opt {
    #[structopt(short, long, env = "JQDATA_MOB")]
    mob: String,

    #[structopt(short, long, env = "JQDATA_PWD")]
    pwd: String,

    #[structopt(short, long, env = "JQDATA_FILE")]
    file: Option<String>,

    #[structopt(subcommand)]
    cmd: Command,
}

#[derive(Debug, StructOpt)]
enum Command {
    Insert {
        #[structopt(short, long)]
        table: String,
        #[structopt(short, long)]
        since: Option<String>,
    }
}