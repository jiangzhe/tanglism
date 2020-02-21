mod error;
use error::Error;
type Result<T> = std::result::Result<T, Error>;
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

    fn populate_trade_days(&self) -> Result<()> {
        let max_day = self.max_trade_day()?;
        if let Some(ref md) = max_day {
            if md < &Local::today() {
                return self.populate_trade_days_since(Some(next_day(md)?));
            }
            Ok(())
        } else {
            return self.populate_trade_days_since(None);
        }
    }

    fn max_trade_day(&self) -> Result<Option<Date<Local>>> {
        let mut stmt = self.conn.prepare("SELECT max(_date) FROM trade_days")?;
        let mut rows = stmt.query(params![])?;
        if let Some(row) = rows.next()? {
            let date_str: String = row.get(0)?;
            return Ok(Some(Local.datetime_from_str(&date_str, "%Y-%m-%d")?.date()));
        }
        Ok(None)
    }

    fn populate_trade_days_since(&self, start: Option<Date<Local>>) -> Result<()> {
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

fn next_day(curr_day: &Date<Local>) -> Result<Date<Local>> {
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
        sql.push_str("INSERT INTO trade_days (_date) VALUES (");
        sql.push_str(&day);
        sql.push_str(");\n");
    }
    sql.push_str("END;\n");
    sql
}
