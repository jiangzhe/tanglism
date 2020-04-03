use crate::models::StockPriceTick;
use crate::DbPool;
use crate::{Error, Result};

// find stock tick by given code and tick string
fn find_stock_price_tick(
    pool: &DbPool,
    input_code: &str,
    input_tick: &str,
) -> Result<Option<StockPriceTick>> {
    use crate::schema::stock_price_ticks::dsl::*;
    use diesel::prelude::*;
    if let Some(conn) = pool.try_get() {
        match stock_price_ticks
            .find((input_code, input_tick))
            .first(&conn)
        {
            Ok(rs) => return Ok(Some(rs)),
            Err(diesel::result::Error::NotFound) => return Ok(None),
            Err(err) => return Err(err.into()),
        }
    }
    Err(Error::FailedAcquireDbConn())
}
