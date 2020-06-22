use crate::handlers::stock_prices::ticks;
use crate::handlers::{stocks, tanglism};
use crate::{DbPool, Result};
use chrono::{Local, NaiveDate};
use serde_derive::*;
use tanglism_morph::StrokeConfig;
use tanglism_utils::{LocalTradingTimestamps, TradingDates};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StockChoice {
    pub code: String,
    pub display_name: String,
    pub msci: bool,
    pub hs300: bool,
    pub choice: ChoiceType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChoiceType {
    BuyOne,
    BuyTwo,
    BuyThree,
}

// 先实现寻找一买
pub async fn list_choices(pool: DbPool, days: usize, limit: usize) -> Result<Vec<StockChoice>> {
    let prioritized_stocks = stocks::search_prioritized_stocks(pool.clone()).await?;
    let mut rst = Vec::new();
    let mut n = 0;
    let (start_dt, end_dt) = start_end_dates(days)?;
    for ps in prioritized_stocks {
        let prices = ticks::query_db_prices(
            pool.clone(),
            "30m".to_owned(),
            ps.code.to_owned(),
            start_dt,
            end_dt,
        )
        .await?;
        let pts = tanglism::get_tanglism_partings(&prices)?;
        let sks = tanglism::get_tanglism_strokes(&pts, "30m", StrokeConfig::default())?;
        let sgs = tanglism::get_tanglism_segments(&sks)?;
        if let Some(last_sg) = sgs.last() {
            // 最后一段向下
            if last_sg.start_price() > last_sg.end_price() {
                let prices_1m = ticks::query_db_prices(
                    pool.clone(),
                    "1m".to_owned(),
                    ps.code.to_owned(),
                    last_sg.start_pt.start_ts.date(),
                    last_sg.end_pt.end_ts.date(),
                )
                .await?;
                let pts_1m = tanglism::get_tanglism_partings(&prices_1m)?;
                let sks_1m =
                    tanglism::get_tanglism_strokes(&pts_1m, "1m", StrokeConfig::default())?;
                let sgs_1m = tanglism::get_tanglism_segments(&sks_1m)?;
                let sts_1m = tanglism::get_tanglism_subtrends(&sgs_1m, &sks_1m, "1m", 1)?;
                let cts_1m = tanglism::get_tanglism_centers(&sts_1m)?;
                // 存在两个中枢
                if cts_1m.len() >= 2 {
                    rst.push(StockChoice {
                        code: ps.code,
                        display_name: ps.display_name,
                        msci: ps.msci,
                        hs300: ps.hs300,
                        choice: ChoiceType::BuyOne,
                    });
                    n += 1;
                    if n >= limit {
                        break;
                    }
                }
            }
        }
    }
    Ok(rst)
}

fn start_end_dates(days: usize) -> Result<(NaiveDate, NaiveDate)> {
    let yesterday = Local::today().naive_local() - chrono::Duration::days(1);
    let tts = LocalTradingTimestamps::new("1d").unwrap();
    let end_dt = if tts.contains_day(yesterday) {
        yesterday
    } else {
        tts.prev_day(yesterday).unwrap()
    };
    let mut start_dt = end_dt;
    for _ in 1..days {
        start_dt = tts.prev_day(start_dt).unwrap();
    }
    Ok((start_dt, end_dt))
}
