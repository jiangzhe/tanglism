//! Command line tool of tanglism stock analysis

use dotenv::dotenv;
use std::env;
use structopt::StructOpt;
use tanglism_web::{Result, DbPool, parse_jqaccount};
use jqdata::*;
use diesel::pg::PgConnection;
use diesel::r2d2::{self, ConnectionManager};
use std::time::Duration;
use tanglism_web::handlers::stocks::search_keyword_stocks;
use tanglism_web::handlers::stock_prices::get_stock_tick_prices;
use tanglism_utils::parse_ts_from_str;
use async_trait::async_trait;
use tokio::sync::Mutex;
use std::sync::Mutex as StdMutex;
use chrono::Local;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    let opt = ToolOpt::from_args();
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

    let mut tool = Tool::new(dburl, jqaccount);
    tool.exec(opt.cmd).await?;
    Ok(())
}

#[derive(Debug, StructOpt)]
#[structopt(name = "tanglism-web", about = "command to run tanglism web server")]
pub struct ToolOpt {
    #[structopt(short, long, help = "specify dbfile to use")]
    dburl: Option<String>,
    #[structopt(short, long, help = "specify jqdata account to use")]
    jqaccount: Option<String>,
    #[structopt(subcommand)]
    cmd: ToolCmd,
}

#[derive(Debug, StructOpt)]
pub enum ToolCmd {
    Count,
    Stock {
        code: String,
    },
    Price {
        code: String,
        tick: String,
        #[structopt(short, long, help = "specify start time of this query")]
        start: String,
        #[structopt(short, long, help = "specify end time of this query")]
        end: Option<String>,
    },
    Autofill,
}

pub struct Tool {
    dburl: String,
    jqaccount: String,
    db: StdMutex<Option<DbPool>>,
    jq: Mutex<Option<JqdataClient>>,
}

impl Tool {
    pub fn new(dburl: String, jqaccount: String) -> Self {
        Tool{ dburl, jqaccount, db: StdMutex::new(None), jq: Mutex::new(None) }
    }

    async fn jq(&self) -> Result<JqdataClient> {
        let mut lock = self.jq.lock().await;
        match &*lock {
            Some(jq) => Ok(jq.clone()),
            None => {
                let (jqmob, jqpwd) = parse_jqaccount(&self.jqaccount)?;
                let jq = JqdataClient::with_credential(jqmob, jqpwd).await?;
                lock.replace(jq);
                Ok(lock.as_ref().unwrap().clone())
            }
        }
    }

    fn db(&self) -> Result<DbPool> {
        let mut lock = self.db.lock().unwrap();
        match &*lock {
            Some(db) => Ok(db.clone()),
            None => {
                let manager = ConnectionManager::<PgConnection>::new(&self.dburl);
                let db = r2d2::Pool::builder()
                    .connection_timeout(Duration::from_secs(3))
                    .build(manager)?;
                lock.replace(db);
                Ok(lock.as_ref().unwrap().clone())
            }
        }
    }
}

#[async_trait]
pub trait ToolCmdExec {
    async fn exec(&mut self, cmd: ToolCmd) -> Result<()>;
}

#[async_trait]
impl ToolCmdExec for Tool {
    async fn exec(&mut self, cmd: ToolCmd) -> Result<()> {
        match cmd {
            ToolCmd::Count => {
                let count = self.jq().await?.execute(GetQueryCount{}).await?;
                println!("{}", count);
            }
            ToolCmd::Stock { code } => {
                let stocks = search_keyword_stocks(self.db()?, code).await?;
                for s in &stocks {
                    println!("{:15}{:15}{:15}", s.code, s.display_name, s.end_date);
                }
            }
            ToolCmd::Price { code, tick, start, end } => {
                let (start_ts, _) = parse_ts_from_str(&start)?;
                let end_ts: chrono::NaiveDateTime = if let Some(end_str) = end.as_ref() {
                    let (ts, _) = parse_ts_from_str(&end_str)?;
                    ts
                } else {
                    let local_ts = Local::today().and_hms(0, 0, 0) - chrono::Duration::seconds(1);
                    local_ts.naive_local()
                };
                let db = self.db()?;
                let jq = &self.jq().await?;
                let prices = get_stock_tick_prices(&db, &jq, &tick, &code, start_ts, end_ts).await?;
                for p in &prices {
                    println!("{:21}{:8.2}{:8.2}{:8.2}{:8.2}{:18.2}{:18.2}", p.ts, p.open, p.close, p.high, p.low, p.volume, p.amount);
                }
            }
            _ => unimplemented!("other commands are not implemented"),
        }
        Ok(())
    }
}
