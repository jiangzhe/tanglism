CREATE TABLE IF NOT EXISTS stock_price_ticks (
    tick VARCHAR(32) NOT NULL,
    code VARCHAR(32) NOT NULL,
    start_dt DATE NOT NULL,
    end_dt DATE NOT NULL,
    PRIMARY KEY (tick, code)
);
