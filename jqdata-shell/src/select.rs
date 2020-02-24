use crate::{code_autocomplete, Result};
use chrono::{Local, NaiveDate};
use rusqlite::{params, Connection};
// use serde::{Deserialize, Serialize};
use serde_derive::*;

#[derive(Debug, Serialize, Deserialize)]
pub struct Price {
    pub date: String,
    pub open: f64,
    pub close: f64,
    pub high: f64,
    pub low: f64,
    pub volume: f64,
    pub money: f64,
}

pub fn select_price_period_1d(
    conn: &mut Connection,
    code: &str,
    from: Option<String>,
    to: Option<String>,
) -> Result<Vec<Price>> {
    let code = code_autocomplete(code)?;
    let from_day = match from {
        None => default_from_day(),
        Some(ref s) => NaiveDate::parse_from_str(s, "%Y-%m-%d")?,
    };
    let to_day = match to {
        None => default_to_day(),
        Some(ref s) => NaiveDate::parse_from_str(s, "%Y-%m-%d")?,
    };
    return select_price_period_1d_range(conn, &code, from_day, to_day);
}

fn select_price_period_1d_range(
    conn: &mut Connection,
    code: &str,
    from: NaiveDate,
    to: NaiveDate,
) -> Result<Vec<Price>> {
    let mut stmt = conn.prepare(
        "SELECT _date, open, close, high, low, volume, money FROM stock_prices_1d \
        where code = ?1 \
        and _date >= ?2 \
        and _date <= ?3 \
        order by _date",
    )?;
    let price_iter = stmt.query_map(
        params![
            code,
            from.format("%Y-%m-%d").to_string(),
            to.format("%Y-%m-%d").to_string()
        ],
        |row| {
            Ok(Price {
                date: row.get(0)?,
                open: row.get(1)?,
                close: row.get(2)?,
                high: row.get(3)?,
                low: row.get(4)?,
                volume: row.get(5)?,
                money: row.get(6)?,
            })
        },
    )?;
    let mut prices = Vec::new();
    for price in price_iter {
        prices.push(price?);
    }
    Ok(prices)
}

// one week age as default from day
fn default_from_day() -> NaiveDate {
    Local::today()
        .naive_local()
        .checked_sub_signed(chrono::Duration::days(7))
        .unwrap()
}

fn default_to_day() -> NaiveDate {
    Local::today().naive_local()
}
