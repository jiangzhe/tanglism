mod error;
pub use error::Error;
pub type Result<T> = std::result::Result<T, Error>;
use rusqlite::{params, Connection};
use jqdata::JqdataClient;
use chrono::prelude::*;

pub struct DatabasePopulator<'db, 'cli> {
    conn: &'db Connection,
    cli: &'cli JqdataClient,
}

impl<'db, 'cli> DatabasePopulator<'db, 'cli> {
    pub fn new(conn: &'db Connection, cli: &'cli JqdataClient) -> Self {
        DatabasePopulator{conn, cli}
    }

    pub fn populate_trade_days(&self, since: Option<String>) -> Result<()> {
        let max_day = self.max_trade_day()?;
        if let Some(md) = max_day {
            let md = if since.is_none() {
                md
            } else {
                let sd = NaiveDate::parse_from_str(&since.unwrap(), "%Y-%m-%d")?;
                if sd > md {
                    md
                } else {
                    sd
                }
            };
            if md < Local::today().naive_local() {
                return self.populate_trade_days_since(Some(next_day(&md)?));
            }
            Ok(())
        } else {
            let since_day = match since {
                Some(s) => Some(NaiveDate::parse_from_str(&s, "%Y-%m-%d")?),
                None => None,
            };
            return self.populate_trade_days_since(since_day);
        }
    }

    fn max_trade_day(&self) -> Result<Option<NaiveDate>> {
        let mut stmt = self.conn.prepare("SELECT max(_date) FROM trade_days")?;
        let mut rows = stmt.query(params![])?;
        if let Some(row) = rows.next()? {
            if row.get_raw(0) == rusqlite::types::ValueRef::Null {
                return Ok(None);
            }
            let date_str: String = row.get(0)?;
            return Ok(Some(NaiveDate::parse_from_str(&date_str, "%Y-%m-%d")?));
        }
        Ok(None)
    }

    fn populate_trade_days_since(&self, start: Option<NaiveDate>) -> Result<()> {
        match start {
            Some(dt) => {
                let date = dt.format("%Y-%m-%d").to_string();
                
                let days = self.cli.execute(jqdata::GetTradeDays{
                    date,
                    end_date: None,
                })?;
                if days.is_empty() {
                    return Ok(());
                }
                self.conn.execute_batch(&batch_trade_days(days))?;
                Ok(())
            }
            None => {
                let days = self.cli.execute(jqdata::GetAllTradeDays{})?;
                if days.is_empty() {
                    return Ok(());
                }
                self.conn.execute_batch(&batch_trade_days(days))?;
                Ok(())
            }
        }
    }
}

fn next_day(curr_day: &NaiveDate) -> Result<NaiveDate> {
    let next_day = curr_day.checked_add_signed(chrono::Duration::days(1));
    if next_day.is_none() {
        return Err(Error("next day overflow".to_owned()));
    }
    Ok(next_day.unwrap())
}

fn batch_trade_days(days: Vec<String>) -> String {
    let mut sql: String = String::new();
    sql.push_str("BEGIN;\n");
    for day in days {
        sql.push_str("INSERT INTO trade_days (_date) VALUES ('");
        sql.push_str(&day);
        sql.push_str("');\n");
    }
    sql.push_str("END;\n");
    sql
}
