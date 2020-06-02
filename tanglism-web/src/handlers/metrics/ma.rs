use super::Metric;
use bigdecimal::BigDecimal;
use chrono::NaiveDateTime;

pub fn ma<D, P, T>(raw: &[D], period: usize, pf: P, tf: T) -> Vec<Metric> 
where 
    P: Fn(&D) -> BigDecimal,
    T: Fn(&D) -> NaiveDateTime,
{
    if raw.len() < period {
        return Vec::new();
    }
    if period == 1 {
        return raw.iter().map(|d| Metric{ts: tf(d), value: pf(d)}).collect();
    }
    let pv = BigDecimal::from(period as u64);
    let mut acc: BigDecimal = raw.iter().take(period).map(&pf).sum();
    let mut res = Vec::with_capacity(raw.len() - period + 1);
    res.push(Metric{
        ts: tf(&raw[period]),
        value: &acc / &pv,
    });
    for (d0, d1) in raw.iter().zip(raw.iter().skip(period)) {
        acc -= (&pf)(d0);
        acc += (&pf)(d1);
        res.push(Metric{
            ts: tf(d1),
            value: &acc / &pv,
        });
    }
    res
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    #[test]
    fn test_ma() {
        let dataset1 = vec![1, 1, 1, 1, 1];
        let ma1 = ma(&dataset1, 3, |d| BigDecimal::from(*d as i64), |_| mock_ts());
        assert_eq!(3, ma1.len());
        for m in &ma1 {
            assert_eq!(BigDecimal::from(1), m.value);
        }
        let dataset2 = vec![1,2,3,4,5,6,7];
        let ma2 = ma(&dataset2, 3, |d| BigDecimal::from(*d as i64), |_| mock_ts());
        assert_eq!(5, ma2.len());
        for (expect, actual) in vec![2, 3, 4, 5, 6].into_iter().zip(ma2.into_iter()) {
            assert_eq!(BigDecimal::from(expect), actual.value);
        }
    }

    fn mock_ts() -> NaiveDateTime {
        NaiveDate::from_ymd(2020, 2, 10).and_hms(15, 0, 0)
    }
}