//! Command line tool of tanglism stock analysis

use async_trait::async_trait;
use bigdecimal::BigDecimal;
use chrono::{Local, NaiveDate};
use diesel::pg::PgConnection;
use diesel::r2d2::{self, ConnectionManager};
use dotenv::dotenv;
use jqdata::*;
use lazy_static::lazy_static;
use std::env;
use std::sync::Mutex as StdMutex;
use std::time::Duration;
use structopt::StructOpt;
use tanglism_utils::{parse_ts_from_str, LocalTradingTimestamps, TradingDates};
use tanglism_web::handlers::metrics;
use tanglism_web::handlers::stock_prices::ticks;
use tanglism_web::handlers::stocks::Stock;
use tanglism_web::handlers::{stock_prices, stocks};
use tanglism_web::{parse_jqaccount, DbPool, Result};
use tokio::sync::Mutex;

lazy_static! {
    static ref AUTOFILL_START_DATE: NaiveDate = NaiveDate::from_ymd(2020, 1, 1);
}

const AUTOFILL_RESERVE_API_COUNT: i32 = 100_000;
const AUTOFILL_BATCH_SIZE_THRESHOLD: i32 = 5000;

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
        #[structopt(
            short,
            long,
            help = "specify tick for autofill, by default 1m",
            default_value = "1m"
        )]
        tick: String,
        #[structopt(
            short,
            long,
            help = "specify iterations for autofill, by default 100",
            default_value = "100"
        )]
        iteration: usize,
    },
    Msci {
        #[structopt(long, help = "specify ATR percentage metric period in days")]
        atrp_days: Option<usize>,
        #[structopt(long, help = "specify the column to sort by, 'max', 'min', 'avg'")]
        sort_by: Option<String>,
    },
    Hs300 {
        #[structopt(long, help = "specify ATR percentage metric period in days")]
        atrp_days: Option<usize>,
        #[structopt(long, help = "specify the column to sort by, 'max', 'min', 'avg'")]
        sort_by: Option<String>,
    },
}

pub struct Tool {
    dburl: String,
    jqaccount: String,
    db: StdMutex<Option<DbPool>>,
    jq: Mutex<Option<JqdataClient>>,
}

impl Tool {
    pub fn new(dburl: String, jqaccount: String) -> Self {
        Tool {
            dburl,
            jqaccount,
            db: StdMutex::new(None),
            jq: Mutex::new(None),
        }
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
            let count = self.jq().await?.execute(GetQueryCount {}).await?;
            log::debug!("Reserved API capacity {}", count);
        }
        Ok(())
    }

    fn show_stocks(&mut self, rs: Vec<Stock>) -> Result<()> {
        // 无需统计ATRP
        println!("{:<15}{:<15}", "CODE", "NAME");
        for s in &rs {
            println!("{:<15}{:<15}", s.code, s.display_name);
        }
        Ok(())
    }

    async fn show_stocks_with_atrp(
        &mut self,
        rs: Vec<Stock>,
        atrp_days: usize,
        sort_by: Option<String>,
    ) -> Result<()> {
        let db = self.db()?;
        let codes = rs.iter().map(|s| s.code.clone()).collect();
        let tick = "1d".to_owned();
        let today = Local::today().naive_local();
        let tts = LocalTradingTimestamps::new("1d").unwrap();
        let end_dt = if tts.contains_day(today) {
            today
        } else {
            tts.prev_day(today).unwrap()
        };
        let mut start_dt = end_dt;
        for _ in 1..atrp_days {
            start_dt = tts.prev_day(start_dt).unwrap();
        }
        let data = ticks::query_db_multiple_prices(db, tick, codes, start_dt, end_dt).await?;
        let atrp_stats = metrics::multi_atrp_stats(&data);
        if let Some(sort_stmt) = sort_by {
            // 需要排序
            // record: code, name, atrp-max, atrp-min, atrp-avg
            let (has_stats, no_stats): (Vec<Stock>, Vec<Stock>) = rs
                .into_iter()
                .partition(|r| atrp_stats.contains_key(&r.code));
            let one_hundred = BigDecimal::from(100);
            let mut has_stats: Vec<_> = has_stats
                .into_iter()
                .map(|s| {
                    let stats = &atrp_stats[&s.code];
                    (
                        s.code,
                        s.display_name,
                        &stats.max * &one_hundred,
                        &stats.min * &one_hundred,
                        &stats.avg * &one_hundred,
                    )
                })
                .collect();
            match sort_stmt.as_ref() {
                "max" | "max-" => has_stats.sort_by(|a, b| b.2.cmp(&a.2)),
                "max+" => has_stats.sort_by(|a, b| a.2.cmp(&b.2)),
                "min" | "min-" => has_stats.sort_by(|a, b| b.3.cmp(&a.3)),
                "min+" => has_stats.sort_by(|a, b| a.3.cmp(&b.3)),
                "avg" | "avg-" => has_stats.sort_by(|a, b| b.4.cmp(&a.4)),
                "avg+" => has_stats.sort_by(|a, b| a.4.cmp(&b.4)),
                _ => panic!("invalid sort column {}", sort_stmt),
            }
            // 标题
            println!(
                "{:<15}{:<15}{:<15}{:<15}{:<15}",
                "CODE", "NAME", "ATRP-MAX", "ATRP-MIN", "ATRP-AVG"
            );
            // 有统计值
            for s in has_stats {
                println!(
                    "{:<15}{:15}{:<15.2}{:<15.2}{:<15.2}",
                    s.0, s.1, s.2, s.3, s.4
                );
            }
            // 无统计值
            for s in &no_stats {
                println!("{:<15}{:<15}", s.code, s.display_name);
            }
            return Ok(());
        }
        // 无需排序
        println!(
            "{:<15}{:<15}{:<15}{:<15}{:<15}",
            "CODE", "NAME", "ATRP-MAX", "ATRP-MIN", "ATRP-AVG"
        );
        let one_hundred = BigDecimal::from(100);
        for s in &rs {
            if let Some(stats) = atrp_stats.get(&s.code) {
                println!(
                    "{:<15}{:15}{:<15.2}{:<15.2}{:<15.2}",
                    s.code,
                    s.display_name,
                    &stats.max * &one_hundred,
                    &stats.min * &one_hundred,
                    &stats.avg * &one_hundred
                );
            } else {
                println!("{:<15}{:<15}", s.code, s.display_name);
            }
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
                let count = self.jq().await?.execute(GetQueryCount {}).await?;
                println!("{}", count);
            }
            ToolCmd::Stock { code } => {
                let rs = stocks::search_keyword_stocks(self.db()?, code).await?;
                for s in &rs {
                    println!("{:15}{:15}{:15}", s.code, s.display_name, s.end_date);
                }
            }
            ToolCmd::Msci { atrp_days, sort_by } => {
                let rs = stocks::search_msci_stocks(self.db()?).await?;
                if let Some(atrp_days) = atrp_days {
                    self.show_stocks_with_atrp(rs, atrp_days, sort_by).await?;
                } else {
                    self.show_stocks(rs)?;
                }
            }
            ToolCmd::Hs300 { atrp_days, sort_by } => {
                let rs = stocks::search_msci_stocks(self.db()?).await?;
                if let Some(atrp_days) = atrp_days {
                    self.show_stocks_with_atrp(rs, atrp_days, sort_by).await?;
                } else {
                    self.show_stocks(rs)?;
                }
            }
            ToolCmd::Price {
                code,
                tick,
                start,
                end,
            } => {
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
                let prices =
                    stock_prices::get_stock_tick_prices(&db, &jq, &tick, &code, start_ts, end_ts)
                        .await?;
                for p in &prices {
                    println!(
                        "{:21}{:8.2}{:8.2}{:8.2}{:8.2}{:18.2}{:18.2}",
                        p.ts, p.open, p.close, p.high, p.low, p.volume, p.amount
                    );
                }
            }
            ToolCmd::Autofill { tick, iteration } => {
                // 从MSCI成分股中选取最近10天内没有行情的，查询并插入数据库
                let msci_stocks = stocks::search_prioritized_stocks(self.db()?).await?;
                let tts = LocalTradingTimestamps::new("1d").unwrap();
                let last_trade_day = tts
                    .prev_day(Local::today().naive_local())
                    .expect("last trade day not exists");
                let mut it = 0;
                for s in &msci_stocks {
                    match stock_prices::query_db_period(&self.db()?, &tick, &s.code).await? {
                        Some(spt) => {
                            if spt.end_dt < last_trade_day {
                                log::info!(
                                    "Stock {} {} has data from {} to {}",
                                    s.code,
                                    tick,
                                    spt.start_dt,
                                    spt.end_dt
                                );
                                let start_dt =
                                    tts.next_day(spt.end_dt).expect("start date not exists");
                                log::info!(
                                    "Try fill stock from {} to {}",
                                    start_dt,
                                    last_trade_day
                                );
                                let mut saf = StockAutofill::new(
                                    self.jq().await?,
                                    self.db()?,
                                    &tick,
                                    &s.code,
                                    start_dt,
                                    last_trade_day,
                                );
                                loop {
                                    if saf.finished() {
                                        log::info!("Stock {} {} autofill finished", s.code, tick);
                                        break;
                                    }
                                    let count = self.jq().await?.execute(GetQueryCount {}).await?;
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
                                log::info!("Stock {} {} has full data", s.code, tick);
                            }
                        }
                        None => {
                            log::info!("Stock {} {} has no data", s.code, tick);
                            let start_dt = *AUTOFILL_START_DATE;
                            log::info!("Try fill stock from {} to {}", start_dt, last_trade_day);
                            let mut saf = StockAutofill::new(
                                self.jq().await?,
                                self.db()?,
                                &tick,
                                &s.code,
                                start_dt,
                                last_trade_day,
                            );
                            loop {
                                if saf.finished() {
                                    log::info!("Stock {} {} autofill finished", s.code, tick);
                                    break;
                                }
                                let count = self.jq().await?.execute(GetQueryCount {}).await?;
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
    records_per_day: i32,
}

impl StockAutofill {
    pub fn new<T: Into<String>, C: Into<String>>(
        jq: JqdataClient,
        db: DbPool,
        tick: T,
        code: C,
        start_dt: NaiveDate,
        end_dt: NaiveDate,
    ) -> Self {
        let tick = tick.into();
        let records_per_day = match tick.as_ref() {
            "1m" => 240,
            "5m" => 48,
            "30m" => 8,
            "1d" => 1,
            _ => panic!("invalid tick {}", tick),
        };
        StockAutofill {
            jq,
            db,
            tick,
            code: code.into(),
            start_dt,
            end_dt,
            records_per_day,
        }
    }

    pub async fn run(&mut self) -> Result<()> {
        if self.finished() {
            return Ok(());
        }

        // 插入数据不可超过AUTOFILL_BATCH_SIZE_THRESHOLD，默认5000
        let tts = LocalTradingTimestamps::new("1d").unwrap();
        let mut it_end = self.start_dt;
        let mut batch_size = self.records_per_day;
        while it_end < self.end_dt
            && batch_size + self.records_per_day < AUTOFILL_BATCH_SIZE_THRESHOLD
        {
            it_end = tts.next_day(it_end).expect("next day not exists");
            batch_size += self.records_per_day;
        }
        // single iteration
        let rs = stock_prices::get_stock_tick_prices(
            &self.db,
            &self.jq,
            &self.tick,
            &self.code,
            self.start_dt.and_hms(0, 0, 0),
            it_end.and_hms(23, 59, 59),
        )
        .await?;
        log::debug!(
            "Fill stock {} {} from {} to {}: {} rows",
            self.code,
            self.tick,
            self.start_dt,
            it_end,
            rs.len()
        );

        self.start_dt = tts.next_day(it_end).expect("next day not exists");
        Ok(())
    }

    pub fn finished(&self) -> bool {
        self.start_dt > self.end_dt
    }
}
