use crate::models::Security;
use crate::schema::securities;
use crate::{DbPool, Result};
use chrono::NaiveDate;
use serde_derive::*;

#[derive(Queryable, Debug, Serialize, Deserialize, Clone)]
pub struct Stock {
    pub code: String,
    pub display_name: String,
    pub name: String,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
}
type StockColumns = (
    securities::code,
    securities::display_name,
    securities::name,
    securities::start_date,
    securities::end_date,
);
const STOCK_COLUMNS: StockColumns = (
    securities::code,
    securities::display_name,
    securities::name,
    securities::start_date,
    securities::end_date,
);

pub async fn search_keyword_stocks(pool: DbPool, keyword: String) -> Result<Vec<Stock>> {
    use crate::schema::securities::dsl::*;
    use diesel::prelude::*;
    // 使用线程池执行阻塞查询
    let rs = tokio::task::spawn_blocking::<_, Result<Vec<Stock>>>(move || {
        let conn = pool.get()?;
        let mut query = securities.filter(tp.eq("stock")).into_boxed();
        if !keyword.is_empty() {
            let code_prefix = format!("{}%", keyword);
            let name_prefix = format!("{}%", keyword);
            let all_match = format!("%{}%", keyword);
            query = query.filter(
                code.ilike(code_prefix)
                    .or(name.ilike(name_prefix).or(display_name.ilike(all_match))),
            );
        }
        let data = query.select(STOCK_COLUMNS).load::<Stock>(&conn)?;
        Ok(data)
    })
    .await??;
    Ok(rs)
}

pub async fn search_msci_stocks(pool: DbPool) -> Result<Vec<Stock>> {
    use crate::schema::securities::dsl::*;
    use diesel::prelude::*;
    let today = chrono::Local::today().naive_local();
    let rs = tokio::task::spawn_blocking::<_, Result<Vec<Stock>>>(move || {
        let conn = pool.get()?;
        let data = securities
            .filter(tp.eq("stock").and(msci.eq(true)).and(end_date.gt(today)))
            .order(code.asc())
            .select(STOCK_COLUMNS)
            .load::<Stock>(&conn)?;
        Ok(data)
    })
    .await??;
    Ok(rs)
}

pub async fn search_hs300_stocks(pool: DbPool) -> Result<Vec<Stock>> {
    use crate::schema::securities::dsl::*;
    use diesel::prelude::*;
    let today = chrono::Local::today().naive_local();
    let rs = tokio::task::spawn_blocking::<_, Result<Vec<Stock>>>(move || {
        let conn = pool.get()?;
        let data = securities
            .filter(tp.eq("stock").and(hs300.eq(true)).and(end_date.gt(today)))
            .order(code.asc())
            .select(STOCK_COLUMNS)
            .load::<Stock>(&conn)?;
        Ok(data)
    })
    .await??;
    Ok(rs)
}

pub async fn search_prioritized_stocks(pool: DbPool) -> Result<Vec<Security>> {
    use crate::schema::securities::dsl::*;
    use diesel::prelude::*;
    let today = chrono::Local::today().naive_local();
    let rs = tokio::task::spawn_blocking::<_, Result<Vec<Security>>>(move || {
        let conn = pool.get()?;
        let data = securities
            .filter(
                tp.eq("stock")
                    .and(msci.eq(true).or(hs300.eq(true)))
                    .and(end_date.gt(today)),
            )
            .order(code.asc())
            .load::<Security>(&conn)?;
        Ok(data)
    })
    .await??;
    Ok(rs)
}
