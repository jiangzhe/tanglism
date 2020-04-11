CREATE TABLE IF NOT EXISTS stock_tick_prices (
    tick VARCHAR(32) NOT NULL,
    code VARCHAR(32) NOT NULL,
    ts TIMESTAMP(0) NOT NULL,
    open NUMERIC(18,4) NOT NULL,
    close NUMERIC(18,4) NOT NULL,
    high NUMERIC(18,4) NOT NULL,
    low NUMERIC(18,4) NOT NULL,
    volume NUMERIC(18,4) NOT NULL,
    amount NUMERIC(18,4) NOT NULL,
    PRIMARY KEY (tick, code, ts)
);
