use super::Metric;
use bigdecimal::BigDecimal;
use chrono::NaiveDateTime;

#[derive(Debug, Clone)]
pub struct AtrInput {
    pub ts: NaiveDateTime,
    pub curr_high: BigDecimal,
    pub curr_low: BigDecimal,
    pub prev_close: BigDecimal,
}

#[derive(Debug, Clone)]
pub struct AtrpStats {
    pub max: BigDecimal,
    pub min: BigDecimal,
    pub avg: BigDecimal,
    pub data: Vec<Metric>,
}

pub fn atr<I>(input: I) -> Vec<Metric>
where
    I: IntoIterator<Item = AtrInput>,
{
    input
        .into_iter()
        .map(|d| {
            // 当日振幅
            let mut p = (&d.curr_high - &d.curr_low).abs();
            // 当日最高价与昨日收盘价的差价
            let p2 = (&d.curr_high - &d.prev_close).abs();
            if p < p2 {
                p = p2;
            }
            // 当日最低价与昨日收盘价的插件
            let p3 = (&d.curr_low - &d.prev_close).abs();
            if p < p3 {
                p = p3;
            }
            Metric { ts: d.ts, value: p }
        })
        .collect()
}

pub fn atrp<I>(input: I) -> Vec<Metric>
where
    I: IntoIterator<Item = AtrInput>,
{
    input
        .into_iter()
        .map(|d| {
            let mut p = (&d.curr_high - &d.curr_low).abs();
            let p2 = (&d.curr_high - &d.prev_close).abs();
            if p < p2 {
                p = p2;
            }
            let p3 = (&d.curr_low - &d.prev_close).abs();
            if p < p3 {
                p = p3;
            }
            Metric {
                ts: d.ts,
                // 使用ATR除以昨日收盘价作为ATR百分比
                value: p / &d.prev_close,
            }
        })
        .collect()
}

pub fn atrp_stats<I>(input: I) -> AtrpStats
where
    I: IntoIterator<Item = AtrInput>,
{
    let data = atrp(input);
    let (sum, count, max, min) = data.iter().fold(
        (
            BigDecimal::from(0),
            0,
            BigDecimal::from(0),
            BigDecimal::from(u32::max_value()),
        ),
        |acc, d| {
            let max = if acc.2 > d.value {
                acc.2
            } else {
                d.value.clone()
            };
            let min = if acc.3 < d.value {
                acc.3
            } else {
                d.value.clone()
            };
            (acc.0 + &d.value, acc.1 + 1, max, min)
        },
    );
    let avg = sum / count;
    AtrpStats {
        max,
        min,
        avg,
        data,
    }
}
