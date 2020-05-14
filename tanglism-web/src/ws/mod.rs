mod session;

use crate::DbPool;
use futures::{FutureExt, StreamExt};
use jqdata::JqdataClient;
use tokio::sync::mpsc;
use warp::filters::BoxedFilter;
use warp::reply::Reply;
use warp::ws::{Message, WebSocket};
use warp::Filter;

pub fn ws_filter(jq: JqdataClient, db: DbPool) -> BoxedFilter<(impl Reply,)> {
    let deps = warp::any().map(move || (jq.clone(), db.clone())).boxed();
    warp::path("ws")
        .and(warp::ws())
        .and(deps)
        .map(|ws: warp::ws::Ws, (jq, db)| {
            ws.on_upgrade(move |socket| start_session(socket, jq, db))
        })
        .boxed()
}

async fn start_session(socket: WebSocket, jq: JqdataClient, db: DbPool) {
    let mut sess = session::Session::new(jq, db);
    log::debug!("Session started");

    let (user_tx, mut user_rx) = socket.split();
    let (tx, rx) = mpsc::unbounded_channel();

    // 转发信息至websocket
    tokio::task::spawn(rx.forward(user_tx).map(|r| {
        if let Err(e) = r {
            log::warn!("websocket send error: {}", e);
        }
    }));

    // 接收用户消息并处理
    while let Some(r) = user_rx.next().await {
        let msg = match r {
            Ok(msg) => msg,
            Err(e) => {
                log::warn!("websocket receive error: {}", e);
                break;
            }
        };
        // 具体逻辑
        if let Ok(s) = msg.to_str() {
            log::debug!("received text message: {}", s);
            match serde_json::from_str(s) {
                Ok(req) => {
                    // 得到响应列表
                    let resp = sess.respond(req).await;
                    let text_resp = serde_json::to_string(&resp).unwrap_or_default();
                    if let Err(e) = tx.send(Ok(Message::text(text_resp))) {
                        log::warn!("internal send error: {}", e);
                    }
                }
                Err(e) => {
                    log::warn!("serde_json error: {}", e);
                    // also send to client
                    let text_resp = serde_json::to_string(&session::Response::Error(e.to_string()))
                        .unwrap_or_default();
                    if let Err(e) = tx.send(Ok(Message::text(text_resp))) {
                        log::warn!("internal send error: {}", e);
                    }
                }
            }
        } else {
            let err_msg = "Non-text user message not supported";
            log::warn!("{}", err_msg);
            // also send to client
            let text_resp = serde_json::to_string(&session::Response::Error(err_msg.to_owned()))
                .unwrap_or_default();
            if let Err(e) = tx.send(Ok(Message::text(text_resp))) {
                log::warn!("internal send error: {}", e);
            }
        }
    }
}
