mod routes;
mod handlers;

use actix::prelude::*;

#[derive(Message)]
#[rtype(result = "()")]
pub struct Message(pub String);

