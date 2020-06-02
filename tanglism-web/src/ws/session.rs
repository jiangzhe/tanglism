use crate::handlers::metrics::{self, MacdMetric};
use crate::handlers::stock_prices::{self, ticks};
use crate::handlers::tanglism;
use crate::BasicCfg;
use crate::{DbPool, Error, ErrorKind, Result};
use jqdata::JqdataClient;
use serde_derive::*;
use std::collections::BTreeSet;
use tanglism_morph::{Center, Segment, Stroke, StrokeConfig, SubTrend};
use tanglism_utils::parse_ts_from_str;

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
#[serde(tag = "type", content = "data")]
pub enum Request {
    BasicCfg {
        tick: String,
        code: String,
        start_dt: String,
        end_dt: String,
    },
    StrokeCfg(String),
    MetricsCfg(String),
    Query {
        refresh: bool,
        objects: Vec<QueryObject>,
        requires: Vec<QueryObject>,
    },
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type", content = "data")]
pub enum Response {
    Ack,
    Error(String),
    Data(Vec<Data>),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type", content = "data")]
pub enum Data {
    KLines(Vec<ticks::StockPrice>),
    KLinesNoChange,
    Strokes(Vec<Stroke>),
    StrokesNoChange,
    Segments(Vec<Segment>),
    SegmentsNoChange,
    SubTrends(Vec<SubTrend>),
    SubTrendsNoChange,
    Centers(Vec<Center>),
    CentersNoChange,
    MACD(MacdMetric),
    MACDNoChange,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize, Clone, PartialOrd, Ord)]
pub enum QueryObject {
    // 笔
    Strokes,
    // 线段
    Segments,
    // 次级别走势
    SubTrends,
    // 中枢
    Centers,
    // MACD指标
    MACD,
}

/// 会话中的临时数据
pub struct Session {
    jq: JqdataClient,
    db: DbPool,
    // 缓存配置
    basic_cfg: Option<BasicCfg>,
    stroke_cfg: Option<StrokeConfig>,
    metrics_cfg: Option<String>,
    // 缓存指标
    ks: Option<Vec<ticks::StockPrice>>,
    strokes: Option<Vec<Stroke>>,
    segments: Option<Vec<Segment>>,
    subtrends: Option<Vec<SubTrend>>,
    centers: Option<Vec<Center>>,
    // DIF/DEA/MACD
    macd: Option<metrics::MacdMetric>,
}

impl Session {
    /// 创建一个新会话
    pub fn new(jq: JqdataClient, db: DbPool) -> Self {
        Session {
            jq,
            db,
            basic_cfg: None,
            stroke_cfg: None,
            metrics_cfg: None,
            ks: None,
            strokes: None,
            segments: None,
            subtrends: None,
            centers: None,
            macd: None,
        }
    }

    /// 处理请求并返回响应
    pub async fn respond(&mut self, req: Request) -> Response {
        match self.do_respond(req).await {
            Ok(resp) => resp,
            Err(e) => Response::Error(e.to_string()),
        }
    }

    async fn do_respond(&mut self, req: Request) -> Result<Response> {
        match req {
            Request::BasicCfg {
                tick,
                code,
                start_dt,
                end_dt,
            } => {
                let (start_ts, _) = parse_ts_from_str(&start_dt)?;
                let (end_ts, _) = parse_ts_from_str(&end_dt)?;
                let new_cfg = BasicCfg {
                    tick,
                    code,
                    start_ts,
                    end_ts,
                };
                let diff = self
                    .basic_cfg
                    .as_ref()
                    .map(|orig| orig != &new_cfg)
                    .unwrap_or(true);
                if diff {
                    log::debug!("replace basic cfg with new one: {:?}", new_cfg);
                    self.basic_cfg.replace(new_cfg);
                    self.clear_k_cache();
                    self.clear_tanglism_cache();
                    self.clear_metrics_cache();
                }
            }
            Request::StrokeCfg(cfg) => {
                let new_cfg = tanglism::parse_stroke_cfg(&cfg)?;
                let diff = self
                    .stroke_cfg
                    .as_ref()
                    .map(|orig| orig != &new_cfg)
                    .unwrap_or(true);
                if diff {
                    log::debug!("replace stroke cfg with new one: {:?}", new_cfg);
                    self.stroke_cfg.replace(new_cfg);
                    self.clear_tanglism_cache();
                }
            }
            Request::MetricsCfg(cfg) => {
                let diff = self
                    .metrics_cfg
                    .as_ref()
                    .map(|orig| orig != &cfg)
                    .unwrap_or(true);
                if diff {
                    log::debug!("replace metrics cfg with new one: {:?}", cfg);
                    self.metrics_cfg.replace(cfg);
                    self.clear_metrics_cache();
                }
            }
            Request::Query {
                refresh,
                objects,
                requires,
            } => {
                if objects.is_empty() {
                    return Ok(Response::Ack);
                }
                let queries = {
                    let mut s = BTreeSet::new();
                    for o in objects {
                        s.insert(o);
                    }
                    s
                };
                let requires = {
                    let mut s = BTreeSet::new();
                    for r in requires {
                        s.insert(r);
                    }
                    s
                };
                let mut dataset = Vec::new();
                // 每次都检查K线
                if self.ensure_ks().await? || refresh {
                    let d = Data::KLines(self.ks.as_ref().cloned().unwrap_or_default());
                    dataset.push(d);
                } else {
                    dataset.push(Data::KLinesNoChange);
                }

                if queries.contains(&QueryObject::Strokes) {
                    if self.ensure_strokes()? || refresh || requires.contains(&QueryObject::Strokes)
                    {
                        let d = Data::Strokes(self.strokes.as_ref().cloned().unwrap_or_default());
                        dataset.push(d);
                    } else {
                        dataset.push(Data::StrokesNoChange);
                    }
                }
                if queries.contains(&QueryObject::Segments) {
                    self.ensure_strokes()?;
                    if self.ensure_segments()?
                        || refresh
                        || requires.contains(&QueryObject::Segments)
                    {
                        let d = Data::Segments(self.segments.as_ref().cloned().unwrap_or_default());
                        dataset.push(d);
                    } else {
                        dataset.push(Data::SegmentsNoChange);
                    }
                }
                if queries.contains(&QueryObject::SubTrends) {
                    if self.ensure_subtrends().await?
                        || refresh
                        || requires.contains(&QueryObject::SubTrends)
                    {
                        let d =
                            Data::SubTrends(self.subtrends.as_ref().cloned().unwrap_or_default());
                        dataset.push(d);
                    } else {
                        dataset.push(Data::SubTrendsNoChange);
                    }
                }
                if queries.contains(&QueryObject::Centers) {
                    self.ensure_subtrends().await?;
                    if self.ensure_centers()? || refresh || requires.contains(&QueryObject::Centers)
                    {
                        let d = Data::Centers(self.centers.as_ref().cloned().unwrap_or_default());
                        dataset.push(d);
                    } else {
                        dataset.push(Data::CentersNoChange);
                    }
                }
                if queries.contains(&QueryObject::MACD) {
                    if self.ensure_macd().await? || refresh || requires.contains(&QueryObject::MACD)
                    {
                        let d = Data::MACD(self.macd.as_ref().cloned().unwrap_or_default());
                        dataset.push(d);
                    } else {
                        dataset.push(Data::MACDNoChange);
                    }
                }
                return Ok(Response::Data(dataset));
            }
        }
        Ok(Response::Ack)
    }

    #[inline]
    fn clear_k_cache(&mut self) {
        self.ks.take();
    }

    #[inline]
    fn clear_tanglism_cache(&mut self) {
        self.strokes.take();
        self.segments.take();
        self.subtrends.take();
        self.centers.take();
    }

    #[inline]
    fn clear_metrics_cache(&mut self) {
        self.macd.take();
    }

    // 检查并更新K线，返回更新标签
    async fn ensure_ks(&mut self) -> Result<bool> {
        if self.ks.is_none() {
            // let ks_params = self.parse_basic_cfg()?;
            if let Some(ref basic_cfg) = self.basic_cfg {
                let ks = stock_prices::get_stock_tick_prices(
                    &self.db,
                    &self.jq,
                    &basic_cfg.tick,
                    &basic_cfg.code,
                    basic_cfg.start_ts,
                    basic_cfg.end_ts,
                )
                .await?;
                self.ks.replace(ks);
                return Ok(true);
            }
        }
        Ok(false)
    }

    // 检查并更新笔，返回更新标签
    fn ensure_strokes(&mut self) -> Result<bool> {
        if self.strokes.is_none() {
            if let Some(ref stroke_cfg) = self.stroke_cfg {
                let tick = match self.basic_cfg {
                    Some(ref bc) => &bc.tick,
                    None => {
                        return Err(Error::custom(
                            ErrorKind::InternalServerError,
                            "basic cfg not exists".to_owned(),
                        ))
                    }
                };
                if let Some(ref ks) = self.ks {
                    let partings = tanglism::get_tanglism_partings(ks)?;
                    let strokes =
                        tanglism::get_tanglism_strokes(&partings, tick, stroke_cfg.clone())?;
                    self.strokes.replace(strokes);
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }

    // 检查并更新线段，返回更新标签
    fn ensure_segments(&mut self) -> Result<bool> {
        if self.segments.is_none() {
            if let Some(ref strokes) = self.strokes {
                let segments = tanglism::get_tanglism_segments(&strokes)?;
                self.segments.replace(segments);
                return Ok(true);
            }
        }
        Ok(false)
    }

    // 检查并更新次级别走势，返回更新标签
    async fn ensure_subtrends(&mut self) -> Result<bool> {
        if self.subtrends.is_none() {
            if let (Some(ref basic_cfg), Some(ref stroke_cfg)) = (&self.basic_cfg, &self.stroke_cfg)
            {
                // 次级别K线
                // 取次级别tick
                let tick = basic_cfg.tick.as_ref();
                let subtick = match tick {
                    "1d" => "30m",
                    "30m" => "5m",
                    "5m" => "1m",
                    "1m" => {
                        return Err(Error::custom(
                            ErrorKind::BadRequest,
                            "tick 1m cannot have subtrends".to_owned(),
                        ))
                    }
                    _ => {
                        return Err(Error::custom(
                            ErrorKind::BadRequest,
                            format!("invalid tick: {}", tick),
                        ))
                    }
                };
                // 无法重用K线是因为级别不同
                let prices = stock_prices::get_stock_tick_prices(
                    &self.db,
                    &self.jq,
                    subtick,
                    &basic_cfg.code,
                    basic_cfg.start_ts,
                    basic_cfg.end_ts,
                )
                .await?;
                let partings = tanglism::get_tanglism_partings(&prices)?;
                let strokes =
                    tanglism::get_tanglism_strokes(&partings, subtick, stroke_cfg.clone())?;
                let segments = tanglism::get_tanglism_segments(&strokes)?;
                let subtrends = tanglism::get_tanglism_subtrends(&segments, &strokes, &tick)?;
                self.subtrends.replace(subtrends);
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn ensure_centers(&mut self) -> Result<bool> {
        if self.centers.is_none() {
            if let Some(ref subtrends) = self.subtrends {
                let centers = tanglism::get_tanglism_centers(subtrends, 1)?;
                self.centers.replace(centers);
                return Ok(true);
            }
        }
        Ok(false)
    }

    async fn ensure_macd(&mut self) -> Result<bool> {
        if self.macd.is_none() {
            log::debug!("macd is none");
            if let Some(ref basic_cfg) = self.basic_cfg {
                log::debug!("basic cfg not null");
                if let Some(ref metrics_cfg) = self.metrics_cfg {
                    log::debug!("metrics cfg not null");
                    let macd_cfg = metrics::parse_macd_cfg(metrics_cfg).unwrap_or_default();
                    log::debug!("macd_cfg={:?}", macd_cfg);
                    let macd = metrics::get_metrics_macd(
                        &self.db,
                        &self.jq,
                        basic_cfg.clone(),
                        macd_cfg.clone(),
                    )
                    .await?;
                    self.macd.replace(macd);
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }
}
