use rusqlite::{Connection, OpenFlags};
use jqdata::JqdataClient;

fn main() {
  
}

fn run() {
    let conn = Connection::open_with_flags("./jqdata.db", 
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE);
    // let client = JqdataClient::with_credential(mob: &str, pwd: &str)

    // let populator = DatabasePopulator{

    // }  
}