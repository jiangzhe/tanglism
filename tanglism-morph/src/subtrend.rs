use crate::shape::{Segment, Stroke, SubTrend, SubTrendType, ValuePoint};
use crate::{Error, Result};
use chrono::NaiveDateTime;

/// 将线段与笔对齐为某个周期下的次级别走势
/// 线段直接视为次级别走势
/// 笔需要进行如下判断
/// 连续1笔：如果1笔存在缺口，视为次级别走势，并标记缺口
///          如果不存在缺口，因为该笔前后必为段，则检查前后两段是否可合并为同向的段
///          如不可以，该笔独立成段，并标记分段。
/// 连续至少2笔：只可能存在两种可能，与前一段合并为同向段，与后一段合并为同向段。
pub fn unify_subtrends(sgs: &[Segment], sks: &[Stroke], tick: &str) -> Result<Vec<SubTrend>> {
    let mut subtrends = Vec::new();
    let mut strokes = Vec::new();
    let mut sgi = 0;
    let mut ski = 0;
    while sgi < sgs.len() {
        let sg = &sgs[sgi];
        // 将线段前的笔加入次级别走势
        strokes.clear();
        while ski < sks.len() && sks[ski].start_pt.extremum_ts < sg.start_pt.extremum_ts {
            let sk = &sks[ski];
            strokes.push(sk.clone());
            if accumulate_strokes(&mut subtrends, &strokes, tick)? {
                strokes.clear();
            }
            ski += 1;
        }
        if strokes.is_empty() {
            // 将线段加入次级别走势
            subtrends.push(segment_as_subtrend(sg, tick)?);
        } else if strokes.len() == 1 {
            let sk = strokes.pop().unwrap();
            subtrends.push(stroke_as_subtrend(&sk, tick, SubTrendType::Divider)?);
            subtrends.push(segment_as_subtrend(sg, tick)?);
        } else {
            // 大于1笔，判断其与后段是否同向
            let sk = strokes.first().unwrap();
            let upward = sk.end_pt.extremum_price > sk.start_pt.extremum_price
                && sg.end_pt.extremum_price > sg.start_pt.extremum_price
                && sg.end_pt.extremum_price > sk.start_pt.extremum_price;
            let downward = sk.end_pt.extremum_price < sk.start_pt.extremum_price
                && sg.end_pt.extremum_price < sg.start_pt.extremum_price
                && sg.end_pt.extremum_price < sk.start_pt.extremum_price;
            if upward || downward {
                // 合并插入
                subtrends.push(SubTrend {
                    start: ValuePoint {
                        ts: align_tick(tick, sk.start_pt.extremum_ts)?,
                        value: sk.start_pt.extremum_price.clone(),
                    },
                    end: ValuePoint {
                        ts: align_tick(tick, sg.end_pt.extremum_ts)?,
                        value: sg.end_pt.extremum_price.clone(),
                    },
                    level: 1,
                    typ: SubTrendType::Combination,
                });
            } else {
                // 忽略笔
                subtrends.push(segment_as_subtrend(sg, tick)?);
            }
        }
        sgi += 1;
        // 跳过所有被线段覆盖的笔
        while ski < sks.len() && sks[ski].start_pt.extremum_ts < sg.end_pt.extremum_ts {
            ski += 1;
        }
    }
    // todo
    Ok(subtrends)
}

fn segment_as_subtrend(sg: &Segment, tick: &str) -> Result<SubTrend> {
    Ok(SubTrend {
        start: ValuePoint {
            ts: align_tick(tick, sg.start_pt.extremum_ts)?,
            value: sg.start_pt.extremum_price.clone(),
        },
        end: ValuePoint {
            ts: align_tick(tick, sg.end_pt.extremum_ts)?,
            value: sg.end_pt.extremum_price.clone(),
        },
        level: 1,
        typ: SubTrendType::Normal,
    })
}

fn stroke_as_subtrend(sk: &Stroke, tick: &str, typ: SubTrendType) -> Result<SubTrend> {
    Ok(SubTrend {
        start: ValuePoint {
            ts: align_tick(tick, sk.start_pt.extremum_ts)?,
            value: sk.start_pt.extremum_price.clone(),
        },
        end: ValuePoint {
            ts: align_tick(tick, sk.end_pt.extremum_ts)?,
            value: sk.end_pt.extremum_price.clone(),
        },
        level: 1,
        typ,
    })
}

// 尝试将增量的笔合并进已存在的次级别走势
fn accumulate_strokes(
    subtrends: &mut Vec<SubTrend>,
    strokes: &[Stroke],
    tick: &str,
) -> Result<bool> {
    if strokes.is_empty() {
        return Ok(false);
    }
    if strokes.len() == 1 {
        // 仅连续1笔
        let sk = strokes.last().unwrap();
        if sk.start_pt.right_gap.is_some() || sk.end_pt.left_gap.is_some() {
            subtrends.push(stroke_as_subtrend(sk, tick, SubTrendType::Gap)?);
            return Ok(true);
        }
    }
    if strokes.len() == 2 {
        let sk = strokes.last().unwrap();
        if let Some(prev_st) = subtrends.last() {
            let upward = prev_st.end.value > prev_st.start.value
                && sk.end_pt.extremum_price > prev_st.start.value;
            let downward = prev_st.end.value < prev_st.start.value
                && sk.end_pt.extremum_price < prev_st.start.value;
            if upward || downward {
                let st = subtrends.last_mut().unwrap();
                st.end.ts = align_tick(tick, sk.end_pt.extremum_ts)?;
                st.end.value = sk.end_pt.extremum_price.clone();
                st.typ = SubTrendType::Combination;
                return Ok(true);
            }
        }
    }
    // 不处理2笔以上情况
    Ok(false)
}

#[inline]
pub(crate) fn align_tick(tick: &str, ts: NaiveDateTime) -> Result<NaiveDateTime> {
    use tanglism_utils::{
        TradingTimestamps, LOCAL_DATES, LOCAL_TS_1_MIN, LOCAL_TS_30_MIN, LOCAL_TS_5_MIN,
    };
    let aligned = match tick {
        "1d" => LOCAL_DATES.aligned_tick(ts),
        "30m" => LOCAL_TS_30_MIN.aligned_tick(ts),
        "5m" => LOCAL_TS_5_MIN.aligned_tick(ts),
        "1m" => LOCAL_TS_1_MIN.aligned_tick(ts),
        _ => {
            return Err(Error(format!("invalid tick: {}", tick)));
        }
    };
    aligned.ok_or_else(|| Error(format!("invalid timestamp: {}", ts)))
}
