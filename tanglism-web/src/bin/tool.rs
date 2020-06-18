//! Command line tool of tanglism stock analysis

use dotenv::dotenv;
use std::env;
use structopt::StructOpt;
use tanglism_web::{Result, DbPool, parse_jqaccount};
use jqdata::*;
use diesel::pg::PgConnection;
use diesel::r2d2::{self, ConnectionManager};
use std::time::Duration;
use tanglism_web::handlers::{stocks, stock_prices};
use tanglism_utils::{parse_ts_from_str, LOCAL_DATES, TradingDates};
use async_trait::async_trait;
use tokio::sync::Mutex;
use std::sync::Mutex as StdMutex;
use chrono::{Local, NaiveDate};
use lazy_static::lazy_static;

lazy_static!{
    static ref AUTOFILL_START_DATE: NaiveDate = NaiveDate::from_ymd(2020, 1, 1);
}

const AUTOFILL_RESERVE_API_COUNT: i32 = 100000;

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
    Autofill {
        #[structopt(short, long, help = "specify tick for autofill, by default 1m", default_value = "1m")]
        tick: String,
        #[structopt(short, long, help = "specify iterations for autofill, by default 100", default_value = "100")]
        iteration: usize,
    },
    Msci,
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

    async fn debug_api_capacity(&mut self) -> Result<()> {
        if log::max_level() >= log::LevelFilter::Debug {
            let count = self.jq().await?.execute(GetQueryCount{}).await?;
            log::debug!("Reserved API capacity {}", count);
        }
        Ok(())
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
                let rs = stocks::search_keyword_stocks(self.db()?, code).await?;
                for s in &rs {
                    println!("{:15}{:15}{:15}", s.code, s.display_name, s.end_date);
                }
            }
            ToolCmd::Msci => {
                let rs = stocks::search_msci_stocks(self.db()?).await?;
                for s in &rs {
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
                let prices = stock_prices::get_stock_tick_prices(&db, &jq, &tick, &code, start_ts, end_ts).await?;
                for p in &prices {
                    println!("{:21}{:8.2}{:8.2}{:8.2}{:8.2}{:18.2}{:18.2}", p.ts, p.open, p.close, p.high, p.low, p.volume, p.amount);
                }
            }
            ToolCmd::Autofill { tick, iteration } => {
                // 从MSCI成分股中选取最近10天内没有行情的，查询并插入数据库
                let msci_stocks = stocks::search_msci_stocks(self.db()?).await?;
                let last_trade_day = LOCAL_DATES.prev_day(Local::today().naive_local()).expect("last trade day not exists");
                let mut it = 0;
                for s in &msci_stocks {
                    match stock_prices::query_db_period(&self.db()?, &tick, &s.code).await? {
                        Some(spt) => {
                            if spt.end_dt < last_trade_day {
                                log::info!("Stock {} {} has data from {} to {}", s.code, tick, spt.start_dt, spt.end_dt);
                                let start_dt = LOCAL_DATES.next_day(spt.end_dt).expect("start date not exists");
                                log::info!("Try fill stock from {} to {}", start_dt, last_trade_day);
                                let mut saf = StockAutofill::new(self.jq().await?, self.db()?, &tick, &s.code, start_dt, last_trade_day);
                                loop {
                                    if saf.finished() {
                                        log::info!("Stock {} {} autofill finished", s.code, tick);
                                        break;
                                    }
                                    let count = self.jq().await?.execute(GetQueryCount{}).await?;
                                    if count < AUTOFILL_RESERVE_API_COUNT {
                                        log::info!("Reached reserved API limit(limit={}, current={}), stop autofill", AUTOFILL_RESERVE_API_COUNT, count);
                                        return Ok(());
                                    } else {
                                        log::info!("JQData API capacity {}", count);
                                    }
                                    saf.run().await?;
                                    it += 1;
                                    if it == iteration {
                                        log::info!("Reached iteration limit, stop autofill");
                                        self.debug_api_capacity().await?;
                                        return Ok(());
                                    }
                                }
                            } else {
                                log::info!("Stock {} {} has full data",  s.code, tick);
                            }
                        }
                        None => {
                            log::info!("Stock {} {} has no data", s.code, tick);
                            let start_dt = *AUTOFILL_START_DATE;
                            log::info!("Try fill stock from {} to {}", start_dt, last_trade_day);
                            let mut saf = StockAutofill::new(self.jq().await?, self.db()?, &tick, &s.code, start_dt, last_trade_day);
                            loop {
                                if saf.finished() {
                                    log::info!("Stock {} {} autofill finished", s.code, tick);
                                    break;
                                }
                                let count = self.jq().await?.execute(GetQueryCount{}).await?;
                                    if count < AUTOFILL_RESERVE_API_COUNT {
                                        log::info!("Reached reserved API limit(limit={}, current={}), stop autofill", AUTOFILL_RESERVE_API_COUNT, count);
                                        return Ok(());
                                    }
                                saf.run().await?;
                                it += 1;
                                if it == iteration {
                                    log::info!("Reached iteration limit, stop autofill");
                                    self.debug_api_capacity().await?;
                                    return Ok(());
                                }
                            }
                        }
                    }
                }
            }
            _ => unimplemented!("other commands are not implemented"),
        }
        Ok(())
    }
}

struct StockAutofill {
    jq: JqdataClient,
    db: DbPool,
    tick: String,
    code: String,
    start_dt: NaiveDate,
    end_dt: NaiveDate,
}

impl StockAutofill {

    pub fn new<T: Into<String>, C: Into<String>>(jq: JqdataClient, db: DbPool, tick: T, code: C, start_dt: NaiveDate, end_dt: NaiveDate) -> Self {
        StockAutofill{
            jq,
            db,
            tick: tick.into(),
            code: code.into(),
            start_dt,
            end_dt,
        }
    }

    pub async fn run(&mut self) -> Result<()> {
        if self.finished() {
            return Ok(());
        }

        // 因为1天的1m数据有240条，按照每次最多20天进行查询和插入，总数不可超过5000条
        let mut it_end = self.start_dt;
        let mut it_days = 1;
        while it_end < self.end_dt {
            it_end = LOCAL_DATES.next_day(it_end).expect("next day not exists");
            it_days += 1;
            if it_days == 20 {
                break;
            }
        }
        // single iteration
        let rs = stock_prices::get_stock_tick_prices(&self.db, &self.jq, &self.tick, &self.code, self.start_dt.and_hms(0, 0, 0), it_end.and_hms(23, 59, 59)).await?;
        log::debug!("Fill stock {} {} from {} to {}: {} rows", self.code, self.tick, self.start_dt, it_end, rs.len());
        
        self.start_dt = LOCAL_DATES.next_day(it_end).expect("next day not exists");
        Ok(())
    }

    pub fn finished(&self) -> bool {
        self.start_dt > self.end_dt
    }
}
