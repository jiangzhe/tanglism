mod error;
use error::Error;
type Result = std::result::Result<(), Error>;
use rusqlite::Connection;
use jqdata::JqdataClient;

pub struct DatabasePopulator<'db, 'cli> {
    conn: &'db Connection,
    cli: &'cli JqdataClient,
}

impl<'db, 'cli> DatabasePopulator<'db, 'cli> {
    pub fn new(conn: &'db Connection, cli: &'cli JqdataClient) -> Self {
        DatabasePopulator{conn, cli}
    }

    fn populate_trade_days(&self) -> Result {
        unimplemented!()
    }
}
