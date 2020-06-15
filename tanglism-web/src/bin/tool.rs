//! Command line tool of tanglism stock analysis

use dotenv::dotenv;
use std::env;
use structopt::StructOpt;
use tanglism_web::{Result, DbPool, parse_jqaccount};
use jqdata::*;
use diesel::pg::PgConnection;
use diesel::r2d2::{self, ConnectionManager};
use std::time::Duration;
use tanglism_web::handlers::stocks::search_keyword_stocks;
use async_trait::async_trait;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    let opt = ToolOpt::from_args();
    dotenv().ok();

    let dburl = if let Some(url) = opt.dburl {
        url
    } else {
        env::var("DATABASE_URL").expect("DATABASE_URL should not be empty")
    };
    let jqaccount = if let Some(account) = opt.jqaccount {
        account
    } else {
        env::var("JQDATA_ACCOUNT").expect("JQDATA_ACCOUNT should not be empty")
    };

    let tool = Tool::new(&dburl, &jqaccount).await?;
    tool.exec(opt.cmd).await?;
    Ok(())
}

#[derive(Debug, StructOpt)]
#[structopt(name = "tanglism-web", about = "command to run tanglism web server")]
pub struct ToolOpt {
    #[structopt(short, long, help = "specify dbfile to use")]
    dburl: Option<String>,
    #[structopt(short, long, help = "specify jqdata account to use")]
    jqaccount: Option<String>,
    #[structopt(subcommand)]
    cmd: ToolCmd,
}

#[derive(Debug, StructOpt)]
pub enum ToolCmd {
    Count,
    Query {
        code: String,
        tick: String,
        #[structopt(short, long, help = "specify start time of this query")]
        start: String,
        #[structopt(short, long, help = "specify end time of this query")]
        end: Option<String>,
    },
    Autofill,
}

pub struct Tool {
    db: DbPool,
    jq: JqdataClient,
}

impl Tool {
    pub async fn new(dburl: &str, jqaccount: &str) -> Result<Self> {
        let manager = ConnectionManager::<PgConnection>::new(dburl);
        let db = r2d2::Pool::builder()
            .connection_timeout(Duration::from_secs(3))
            .build(manager)
            .expect("Failed to create db connection pool");
        let (jqmob, jqpwd) = parse_jqaccount(&jqaccount)?;
        let jq = JqdataClient::with_credential(jqmob, jqpwd).await?;
        Ok(Tool{db, jq})
    }
}

#[async_trait]
pub trait ToolCmdExec {
    async fn exec(&self, cmd: ToolCmd) -> Result<()>;
}

#[async_trait]
impl ToolCmdExec for Tool {
    async fn exec(&self, cmd: ToolCmd) -> Result<()> {
        match cmd {
            ToolCmd::Count => {
                let count = self.jq.execute(GetQueryCount{}).await?;
                println!("{}", count);
            }
            ToolCmd::Query { code, tick, start, end } => {
                
            }
            _ => unimplemented!("other commands are not implemented"),
        }
        Ok(())
    }
}
