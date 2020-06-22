table! {
    securities (code) {
        code -> Varchar,
        display_name -> Varchar,
        name -> Varchar,
        start_date -> Date,
        end_date -> Date,
        tp -> Varchar,
        msci -> Bool,
        hs300 -> Bool,
    }
}

table! {
    stock_daily_prices (code, dt) {
        code -> Varchar,
        dt -> Date,
        open -> Numeric,
        close -> Numeric,
        high -> Numeric,
        low -> Numeric,
        volume -> Numeric,
        amount -> Numeric,
    }
}

table! {
    stock_price_ticks (tick, code) {
        tick -> Varchar,
        code -> Varchar,
        start_dt -> Date,
        end_dt -> Date,
    }
}

table! {
    stock_tick_prices (tick, code, ts) {
        tick -> Varchar,
        code -> Varchar,
        ts -> Timestamp,
        open -> Numeric,
        close -> Numeric,
        high -> Numeric,
        low -> Numeric,
        volume -> Numeric,
        amount -> Numeric,
    }
}

table! {
    trade_days (dt) {
        dt -> Date,
    }
}

allow_tables_to_appear_in_same_query!(
    securities,
    stock_daily_prices,
    stock_price_ticks,
    stock_tick_prices,
    trade_days,
);
