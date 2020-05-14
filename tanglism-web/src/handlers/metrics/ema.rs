use super::Metric;
use bigdecimal::BigDecimal;
use chrono::NaiveDateTime;

/// EMA计算
///
/// 给定价格序列，计算该序列EMA指标。
/// 设周期为T，收盘价P(n)，序列下标n从0开始。
/// EMA(0) = P(0)
/// EMA(n) = EMA(n-1) * (T-1) / (T+1) + P(n) * 2 / (T+1)
pub fn approximate_ema<D, P, T>(raw: &[D], period: u32, pf: P, tf: T) -> Vec<Metric>
where
    P: Fn(&D) -> BigDecimal,
    T: Fn(&D) -> NaiveDateTime,
{
    if raw.is_empty() {
        return Vec::new();
    }
    let pm1 = BigDecimal::from(period - 1);
    let pp1 = BigDecimal::from(period + 1);
    let two = BigDecimal::from(2);
    let mut ema = Vec::with_capacity(raw.len());
    let first = raw.first().unwrap();
    ema.push(Metric {
        ts: tf(first),
        value: pf(first),
    });
    for r in raw.iter().skip(1) {
        let ts = tf(r);
        let price = pf(r);
        ema.push(Metric {
            ts,
            value: &ema.last().unwrap().value * &pm1 / &pp1 + &price * &two / &pp1,
        });
    }
    ema
    // let data: Vec<Data<T>> = raw.iter().map(|d| Data{value: f(d), associated: d.clone()}).collect();
    // approximate_ema_data(&data, t)
}

/// DIF/DEA/MACD计算
///
/// 给定价格序列，计算该序列DIF/DEA指标
pub fn approximate_macd<D, P, T>(
    raw: &[D],
    p_fast_ema: u32,
    p_slow_ema: u32,
    p_dea: u32,
    pf: P,
    tf: T,
) -> (Vec<Metric>, Vec<Metric>, Vec<Metric>)
where
    P: Fn(&D) -> BigDecimal,
    T: Fn(&D) -> NaiveDateTime,
{
    if raw.is_empty() {
        return (Vec::new(), Vec::new(), Vec::new());
    }
    let fast_ema = approximate_ema(raw, p_fast_ema, &pf, &tf);
    let slow_ema = approximate_ema(raw, p_slow_ema, &pf, &tf);
    let dif: Vec<Metric> = fast_ema
        .into_iter()
        .zip(slow_ema.into_iter())
        .map(|(f, s)| Metric {
            ts: f.ts,
            value: f.value - s.value,
        })
        .collect();
    let dea = approximate_ema(&dif, p_dea, |m| m.value.clone(), |m| m.ts);

    let two = BigDecimal::from(2);
    let macd: Vec<Metric> = dif
        .iter()
        .zip(dea.iter())
        .map(|(m1, m2)| Metric {
            ts: m1.ts,
            value: (&m1.value - &m2.value) * &two,
        })
        .collect();

    (dif, dea, macd)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::iter;

    #[test]
    fn test_ema_empty() {
        let raw: Vec<(NaiveDateTime, BigDecimal)> = vec![];
        assert!(approximate_ema(&raw, 12, |r| r.1.clone(), |r| r.0).is_empty());
    }

    #[test]
    fn test_ema_same_seq() {
        let prices: Vec<(NaiveDateTime, BigDecimal)> = iter::repeat(10)
            .take(5)
            .map(|i| (mock_ts(), BigDecimal::from(i)))
            .collect();
        let ema = approximate_ema(&prices, 12, |r| r.1.clone(), |r| r.0);
        assert_eq!(5, ema.len());
        for e in &ema {
            assert!(within_epsilon(&e.value, &BigDecimal::from(10), 0.0001));
        }
    }

    #[test]
    fn test_ema_real() {
        let prices: Vec<(NaiveDateTime, BigDecimal)> =
            vec![17.65, 19.42, 21.36, 23.50, 25.85, 24.36, 26.80, 26.02]
                .into_iter()
                .map(|i| (mock_ts(), BigDecimal::from(i)))
                .collect();
        let ema_expected = vec![17.65, 17.92, 18.45, 19.23, 20.25, 20.88, 21.79, 22.44];
        let ema = approximate_ema(&prices, 12, |r| r.1.clone(), |r| r.0);
        assert_eq!(prices.len(), ema.len());
        for (expected, actual) in ema_expected.iter().zip(ema.iter()) {
            assert!(within_epsilon(
                &BigDecimal::from(*expected),
                &actual.value,
                0.005
            ));
        }
    }

    fn mock_ts() -> NaiveDateTime {
        NaiveDateTime::parse_from_str("2020-02-10 15:00", "%Y-%m-%d %H:%M").unwrap()
    }

    fn within_epsilon(d1: &BigDecimal, d2: &BigDecimal, epsilon: f64) -> bool {
        let diff = if d1 < d2 { d2 - d1 } else { d1 - d2 };
        diff < BigDecimal::from(epsilon)
    }
}
