use jqdata::JqdataClient;
use jqdata_shell::Error;
use jqdata_shell::{select_price_period_1d, PricePeriodInserter, TradeDayInserter};
use rusqlite::{Connection, OpenFlags};
use structopt::StructOpt;

fn main() -> std::result::Result<(), Error> {
    let opt = Opt::from_args();

    let mut conn = Connection::open_with_flags(
        opt.file.unwrap_or("./jqdata.db".to_owned()),
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE,
    )?;

    match opt.cmd {
        Command::Insert {
            mob,
            pwd,
            table,
            unit,
            code,
            from,
            to,
        } => {
            let cli = JqdataClient::with_credential(&mob, &pwd)?;
            match &table[..] {
                "trade_days" => {
                    let mut inserter = TradeDayInserter::new(conn, cli);
                    let inserted = inserter.insert(from, to)?;
                    println!("{} rows inserted", inserted);
                }
                "stock_prices" => {
                    if code.is_none() {
                        return Err(Error(
                            "code must be specified for stock_prices_* table".to_owned(),
                        ));
                    }
                    if unit.is_none() {
                        return Err(Error(
                            "unit must be specified for stock_prices_* table".to_owned(),
                        ));
                    }
                    let mut inserter = PricePeriodInserter::new(conn, cli, &unit.unwrap())?;
                    let inserted = inserter.insert_code(&code.unwrap(), from, to)?;
                    println!("{} rows inserted", inserted);
                }
                _ => return Err(Error(format!("unknown table {}", table))),
            }
        }
        Command::Select { code, from, to } => {
            let prices = select_price_period_1d(&mut conn, &code, from, to)?;
            serde_json::to_writer_pretty(std::io::stdout(), &prices)?;
        }
    }
    Ok(())
}

#[derive(Debug, StructOpt)]
#[structopt(name = "jqdata-shell", about = "shell to run jqdata commands")]
struct Opt {
    #[structopt(short, long, env = "JQDATA_FILE")]
    file: Option<String>,

    #[structopt(subcommand)]
    cmd: Command,
}

#[derive(Debug, StructOpt)]
enum Command {
    Insert {
        #[structopt(short, long, env = "JQDATA_MOB")]
        mob: String,
        #[structopt(short, long, env = "JQDATA_PWD")]
        pwd: String,
        #[structopt(short, long)]
        table: String,
        #[structopt(short, long, help = "specify unit if table is stock_prices_* ")]
        unit: Option<String>,
        #[structopt(short, long, help = "specify stock code if table is stock_prices_* ")]
        code: Option<String>,
        #[structopt(short, long)]
        from: Option<String>,
        #[structopt(short, long)]
        to: Option<String>,
    },
    Select {
        code: String,
        #[structopt(short, long)]
        from: Option<String>,
        #[structopt(short, long)]
        to: Option<String>,
    },
}
