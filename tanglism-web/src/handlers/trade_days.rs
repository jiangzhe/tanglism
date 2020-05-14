use crate::{DbPool, Result};
use chrono::NaiveDate;

// get data from db
#[allow(dead_code)]
pub async fn get_trade_days(
    pool: &DbPool,
    start: NaiveDate,
    end: NaiveDate,
) -> Result<Vec<NaiveDate>> {
    use crate::schema::trade_days::dsl::*;
    use diesel::prelude::*;
    let conn = pool.get()?;
    let rs = tokio::task::spawn_blocking(move || {
        trade_days
            .filter(dt.gt(start).and(dt.le(end)))
            .select(dt)
            .load::<NaiveDate>(&conn)
    })
    .await??;
    Ok(rs)
}
