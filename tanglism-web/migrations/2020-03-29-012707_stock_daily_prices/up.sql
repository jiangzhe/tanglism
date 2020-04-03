CREATE TABLE IF NOT EXISTS stock_daily_prices (
    code VARCHAR(32) NOT NULL,
    dt DATE NOT NULL,
    open NUMERIC(18,4) NOT NULL,
    close NUMERIC(18,4) NOT NULL,
    high NUMERIC(18,4) NOT NULL,
    low NUMERIC(18,4) NOT NULL,
    volume NUMERIC(18,4) NOT NULL,
    amount NUMERIC(18,4) NOT NULL,
    PRIMARY KEY (code, dt)
);
