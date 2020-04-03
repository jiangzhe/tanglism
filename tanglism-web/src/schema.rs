table! {
    securities (code) {
        code -> Varchar,
        display_name -> Varchar,
        name -> Varchar,
        start_date -> Date,
        end_date -> Date,
        tp -> Varchar,
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
    stock_price_ticks (code, tick) {
        code -> Varchar,
        tick -> Varchar,
        start_dt -> Date,
        end_dt -> Date,
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
    trade_days,
);
