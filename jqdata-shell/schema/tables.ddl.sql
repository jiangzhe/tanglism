CREATE TABLE IF NOT EXISTS securities (
    code TEXT PRIMARY KEY,
    display_name TEXT,
    name TEXT,
    start_date TEXT,
    end_date TEXT,
    type TEXT
);

CREATE TABLE IF NOT EXISTS security_infos (
    code TEXT PRIMARY KEY,
    display_name TEXT,
    name TEXT,
    start_date TEXT,
    end_date TEXT,
    type TEXT,
    parent TEXT
);

CREATE TABLE IF NOT EXISTS trade_days (
    _date TEXT PRIMARY KEY
);

CREATE TABLE IF NOT EXISTS stock_prices_1d (
    code TEXT,
    _date TEXT,
    open REAL,
    close REAL,
    high REAL,
    low REAL,
    volume REAL,
    money REAL,
    paused INTEGER,
    high_limit REAL,
    low_limit REAL,
    avg REAL,
    pre_close REAL
);

CREATE TABLE IF NOT EXISTS stock_prices_1m (
    code TEXT,
    _date TEXT,
    open REAL,
    close REAL,
    high REAL,
    low REAL,
    volume REAL,
    money REAL,
);
