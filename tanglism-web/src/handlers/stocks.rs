use super::{Paginate, Pagination};
use crate::helpers::respond_json;
use crate::schema::securities;
use crate::{DbPool, Result};
use actix_web::get;
use actix_web::web::{self, Json};
use chrono::NaiveDate;
use serde_derive::*;

#[derive(Debug, Serialize, Deserialize)]
pub struct Param {
    keyword: Option<String>,
    page: Option<i64>,
    page_size: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", content = "content")]
pub enum Response {
    Pagination {
        #[serde(flatten)]
        pagination: Pagination,
        data: Vec<Stock>,
    },
    Raw(Vec<Stock>),
}

const DEFAULT_PAGE_SIZE: i64 = 10;

#[derive(Queryable, Debug, Serialize, Deserialize)]
pub struct Stock {
    code: String,
    display_name: String,
    name: String,
    start_date: NaiveDate,
    end_date: NaiveDate,
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
// type StockSelect = diesel::dsl::Select<securities::table, StockColumns>;

#[get("/keyword-stocks")]
pub async fn api_search_keyword_stocks(
    pool: web::Data<DbPool>,
    web::Query(param): web::Query<Param>,
) -> Result<Json<Response>> {
    let resp = web::block(move || search_keyword_stock(&pool, param)).await?;
    respond_json(resp)
}

pub fn search_keyword_stock(pool: &DbPool, param: Param) -> Result<Response> {
    use crate::schema::securities::dsl::*;
    use diesel::prelude::*;
    let conn = pool.get()?;
    let mut query = securities.filter(tp.eq("stock")).into_boxed();
    if let Some(keyword) = param.keyword {
        let code_prefix = format!("{}%", keyword);
        let name_prefix = format!("{}%", keyword);
        let all_match = format!("%{}%", keyword);
        query = query.filter(
            code.ilike(code_prefix)
                .or(name.ilike(name_prefix).or(display_name.ilike(all_match))),
        );
    }
    if let Some(page) = param.page {
        let page_size = param.page_size.unwrap_or(DEFAULT_PAGE_SIZE);
        let (data, total) = query
            .select(STOCK_COLUMNS)
            .paginate(page, page_size)
            .load_and_count_total::<Stock>(&conn)?;
        return Ok(Response::Pagination {
            pagination: Pagination {
                total,
                page,
                page_size,
            },
            data,
        });
    }
    let rs = query.select(STOCK_COLUMNS).load::<Stock>(&conn)?;
    Ok(Response::Raw(rs))
}
