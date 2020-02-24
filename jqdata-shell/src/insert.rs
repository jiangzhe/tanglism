use crate::{code_autocomplete, request_datetime, Error, Result};
use jqdata::JqdataClient;
use rusqlite::{params, Connection, ToSql};
use crate::datetime::{DatetimeProcessor, DatetimeRange};

type InsertResult = Result<u64>;

// inserter of table trade days
pub struct TradeDayInserter {
    conn: Connection,
    cli: JqdataClient,
    dtp: DatetimeProcessor,
}

impl TradeDayInserter {
    pub fn new(conn: Connection, cli: JqdataClient) -> Self {
        let dtp = DatetimeProcessor::new("1d").unwrap();
        TradeDayInserter{conn, cli, dtp}
    }

    pub fn insert(&mut self, from: Option<String>, to: Option<String>) -> InsertResult {
        let from = from.unwrap_or("2010-01-01".to_owned());
        let to = match to {
            None => self.dtp.end_of_today(),
            Some(to) => to,
        };
        let dt_range = match self.datetime_range()? {
            None => return self.fetch_and_insert(&from, &to),
            Some(dtr) => dtr,
        };
        // no gap is allowed
        if dt_range.min_after(&to)? {
            return Err(Error(format!("gap not allowed, existing min={}, to={}", dt_range.min(), to)));
        }
        if dt_range.max_before(&from)? {
            return Err(Error(format!("gap not allowed, existing max={}, from={}", dt_range.max(), from)));
        }
        let mut inserted = 0;
        if dt_range.min_after(&from)? {
            inserted += self.fetch_and_insert(
                &from,
                &self.dtp.prev(&dt_range.min())?,
            )?;
        }
        if dt_range.max_before(&to)? {
            inserted += self.fetch_and_insert(
                &self.dtp.next(&dt_range.max())?,
                &to,
            )?;
        }
        Ok(inserted)
    }

    fn datetime_range(&self) -> Result<Option<DatetimeRange>> {
        let mut stmt =
            self.conn.prepare("SELECT MIN(_date) as min_date, MAX(_date) as max_date FROM trade_days")?;
        let mut rows = stmt.query(params![])?;
        if let Some(row) = rows.next()? {
            if row.get_raw(0) == rusqlite::types::ValueRef::Null {
                return Ok(None);
            }
            let min: String = row.get(0)?;
            let max: String = row.get(1)?;
            let dt_range = DatetimeRange::new(&min, &max)?;
            return Ok(Some(dt_range));
        }
        Ok(None)
    }

    fn fetch_and_insert(&mut self, from: &str, to: &str) -> InsertResult {
        let days = self.cli.execute(jqdata::GetTradeDays {
            date: request_datetime(from)?,
            end_date: request_datetime(to).ok(),
        })?;
        let inserted = self.batch_insert(days)?;
        Ok(inserted)
    }

    fn batch_insert(&mut self, days: Vec<String>) -> InsertResult {
        let trx = self.conn.transaction()?;
        let mut inserted = 0;
        for day in days {
            let mut stmt = trx.prepare_cached("INSERT INTO trade_days (_date) VALUES (?1)")?;
            stmt.execute(params![&day])?;
            inserted += 1;
        }
        trx.commit()?;
        Ok(inserted)
    }
}

/// inserter of table price period
/// the underlying sqlite tables are separated, in consideration of data size
/// tables are `stock_prices_1d`, `stock_prices_30m`, `stock_prices_5m`, `stock_prices_1m`
pub struct PricePeriodInserter {
    conn: Connection,
    cli: JqdataClient,
    // sql to query max, and min days by given code
    date_range_sql: String,
    // sql to insert prices by given code
    batch_insert_sql: String,
    dtp: DatetimeProcessor,
}

impl PricePeriodInserter {
    pub fn new(conn: Connection, cli: JqdataClient, unit: &str) -> Result<Self> {
        let dtp = DatetimeProcessor::new(unit)?;
        let date_range_sql = format!("SELECT MIN(_date) as min_date, MAX(_date) as max_date FROM stock_prices_{} WHERE code = ?1", unit);
        let batch_insert_sql = format!(
            "INSERT INTO stock_prices_{} ( \
            code, _date, open, close, high, low, volume, money \
            ) VALUES ( \
            ?1,   ?2,    ?3,   ?4,    ?5,   ?6,  ?7,     ?8    )",
            unit
        );
        Ok(PricePeriodInserter {
            conn,
            cli,
            date_range_sql,
            batch_insert_sql,
            dtp,
        })
    }

    pub fn insert_code(
        &mut self,
        code: &str,
        from: Option<String>,
        to: Option<String>,
    ) -> InsertResult {
        let code = code_autocomplete(code)?;
        let from = from.unwrap_or("2010-01-01".to_owned());
        let to = match to {
            None => self.dtp.end_of_today(),
            Some(to) => to,
        };
        let dt_range = match self.datetime_range(&code)? {
            None => return self.fetch_and_insert(&code, &from, &to),
            Some(dtr) => dtr,
        };
        // no gap is allowed
        if dt_range.min_after(&to)? {
            return Err(Error(format!("gap not allowed, existing min={}, to={}", dt_range.min(), to)));
        }
        if dt_range.max_before(&from)? {
            return Err(Error(format!("gap not allowed, existing max={}, from={}", dt_range.max(), from)));
        }
        let mut inserted = 0;
        if dt_range.min_after(&from)? {
            inserted += self.fetch_and_insert(
                &code,
                &from,
                &self.dtp.prev(&dt_range.min())?,
            )?;
        }
        if dt_range.max_before(&to)? {
            inserted += self.fetch_and_insert(
                &code,
                &self.dtp.next(&dt_range.max())?,
                &to,
            )?;
        }
        Ok(inserted)
    }

    fn datetime_range(&self, code: &str) -> Result<Option<DatetimeRange>> {
        let mut stmt = self.conn.prepare(&self.date_range_sql)?;
        let mut rows = stmt.query(params![code])?;
        if let Some(row) = rows.next()? {
            if row.get_raw(0) == rusqlite::types::ValueRef::Null {
                return Ok(None);
            }
            let min: String = row.get(0)?;
            let max: String = row.get(1)?;
            let dt_range = DatetimeRange::new(&min, &max)?;
            return Ok(Some(dt_range));
        }
        Ok(None)
    }

    fn fetch_and_insert(&mut self, code: &str, from: &str, to: &str) -> InsertResult {
        let prices = self.cli.execute(jqdata::GetPricePeriod {
            code: code.to_owned(),
            unit: self.dtp.unit.to_owned(),
            date: request_datetime(from)?,
            end_date: request_datetime(to)?,
            fq_ref_date: None,
        })?;
        let inserted = self.batch_insert(code, prices)?;
        Ok(inserted)
    }

    fn batch_insert(&mut self, code: &str, prices: Vec<jqdata::Price>) -> InsertResult {
        let trx = self.conn.transaction()?;
        let mut inserted = 0;
        for price in prices {
            let mut stmt = trx.prepare_cached(&self.batch_insert_sql)?;
            let mut params: Vec<&dyn ToSql> = Vec::with_capacity(13);
            params.push(&code);
            params.push(&price.date);
            params.push(&price.open);
            params.push(&price.close);
            params.push(&price.high);
            params.push(&price.low);
            params.push(&price.volume);
            params.push(&price.money);
            stmt.execute(&params)?;
            inserted += 1;
        }
        trx.commit()?;
        Ok(inserted)
    }
}



#[cfg(test)]
mod tests {
    use crate::{Error, Result};
    use rusqlite::{Connection, OpenFlags};
    use chrono::NaiveDate;

    #[test]
    fn test_sqlite_batch() -> Result<()> {
        let conn = Connection::open_with_flags(
            "./jqdata-test.db",
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE,
        )?;

        Ok(())
    }

}
