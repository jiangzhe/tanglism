use super::{Paginate, Pagination};
use crate::helpers::respond_json;
use crate::{DbPool, Error, ErrorKind, Result};
use actix_web::web::Json;
use actix_web::{get, web};
use chrono::NaiveDate;
use serde_derive::*;

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", content = "content")]
pub enum Response {
    Pagination {
        #[serde(flatten)]
        pagination: Pagination,
        data: Vec<NaiveDate>,
    },
    Raw(Vec<NaiveDate>),
}

// default page size is 10
const DEFAULT_PAGE_SIZE: i64 = 10;

#[derive(Debug, Serialize, Deserialize)]
pub struct RequestParam {
    #[serde(skip_serializing_if = "Option::is_none")]
    start: Option<NaiveDate>,
    #[serde(skip_serializing_if = "Option::is_none")]
    end: Option<NaiveDate>,
    #[serde(skip_serializing_if = "Option::is_none")]
    page: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    page_size: Option<i64>,
}

#[get("/trade-days")]
pub async fn api_get_trade_days(
    pool: web::Data<DbPool>,
    web::Query(req): web::Query<RequestParam>,
) -> Result<Json<Response>> {
    let resp = web::block(move || get_trade_days(&pool, req)).await?;
    respond_json(resp)
}

// get data from db
pub fn get_trade_days(pool: &DbPool, req: RequestParam) -> Result<Response> {
    use crate::schema::trade_days::dsl::*;
    use diesel::prelude::*;
    if let Some(conn) = pool.try_get() {
        // make boxed query to enable conditional where clause
        let mut query = trade_days.into_boxed();
        if let Some(start) = req.start {
            query = query.filter(dt.ge(start));
        }
        if let Some(end) = req.end {
            query = query.filter(dt.le(end));
        }
        // conditional pagination
        if let Some(page) = req.page {
            let page_size = req.page_size.unwrap_or(DEFAULT_PAGE_SIZE);
            let (data, total) = query
                .select(dt)
                .paginate(page, page_size)
                .load_and_count_total::<NaiveDate>(&conn)?;
            return Ok(Response::Pagination {
                pagination: Pagination {
                    total,
                    page,
                    page_size,
                },
                data,
            });
        }
        let rs = query.select(dt).load::<NaiveDate>(&conn)?;
        return Ok(Response::Raw(rs));
    }
    Err(Error::FailedAcquireDbConn())
}

#[cfg(test)]
mod tests {
    use super::super::Pagination;
    use super::*;

    #[test]
    fn test_trade_days_json_raw() {
        let raw_resp = Response::Raw(vec![]);
        let json = serde_json::to_string(&raw_resp).unwrap();
        assert_eq!(r#"{"type":"Raw","content":[]}"#, json);
    }

    #[test]
    fn test_trade_days_json_pagination() {
        let pg = Pagination {
            total: 0,
            page: 1,
            page_size: 10,
        };
        let data = vec![];
        let pg_resp = Response::Pagination {
            pagination: pg,
            data,
        };
        let json = serde_json::to_string(&pg_resp).unwrap();
        assert_eq!(
            r#"{"type":"Pagination","content":{"total":0,"page":1,"page_size":10,"data":[]}}"#,
            json
        );
    }
}
