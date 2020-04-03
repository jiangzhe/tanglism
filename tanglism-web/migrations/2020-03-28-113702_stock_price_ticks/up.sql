CREATE TABLE IF NOT EXISTS stock_price_ticks (
    code VARCHAR(32) NOT NULL,
    tick VARCHAR(32) NOT NULL,
    start_dt DATE NOT NULL,
    end_dt DATE NOT NULL,
    PRIMARY KEY (code, tick)
);
