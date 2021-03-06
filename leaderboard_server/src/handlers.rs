use crate::schema::{self, *};
use crate::WSConnections_;
use uuid::Uuid;
use crate::types::*;
use warp_ws_server::{WSMsgOut, BoxError};
use crate::subscriptions::*;
use crate::db;
use crate::messages::*;
use diesel_utils::*;
use crate::diesel::RunQueryDsl;  // imported here so that can run db macros
use crate::diesel::ExpressionMethods;
use warp_ws_server::{GetEz, sub, unsub, sub_all, publish};
use std::collections::HashMap;
use itertools::Itertools;

pub async fn sub_leagues(method: &str, message_id: Uuid, data: SubLeague, conn: PgConn, ws_conns: &mut WSConnections_, user_ws_id: Uuid) -> Result<String, BoxError>{
    let mut hmmmm = ws_conns.lock().await;
    let ws_user = hmmmm.get_mut(&user_ws_id).ok_or("Websocket gone away")?;
    let sub_type = SubType::League;
    if let Some(toggle) = data.all{
        sub_all(&sub_type, ws_user, toggle).await;
    }
    if let Some(ids) = data.sub_league_ids{
        sub(&sub_type, ws_user, ids.iter()).await;
    }
    if let Some(ids) = data.unsub_league_ids{
        unsub(&sub_type, ws_user, ids.iter()).await;
    }
    let subscription = ws_user.subscriptions.get_ez(&SubType::League);
    let data = match subscription.all{
        true => {
            db::latest_leaderboards(&conn, None, None)
        },
        false => {
            db::latest_leaderboards(&conn, Some(subscription.ids.iter().collect()), None)
        }
    }?;
    let resp_msg = WSMsgOut::resp(message_id, method, data);
    serde_json::to_string(&resp_msg).map_err(|e| e.into())
}

pub async fn sub_leaderboards(method: &str, message_id: Uuid, data: SubLeaderboard, conn: PgConn, ws_conns: &mut WSConnections_, user_ws_id: Uuid) -> Result<String, BoxError>{
    // let ws_user = ws_conns.lock().await.get_mut(&user_ws_id).ok_or("Webscoket gone away")?;
    // why does this need splitting into two lines?
    // ANd is it holding the lock for this whole scope? doesnt need to
    let mut hmmmm = ws_conns.lock().await;
    let ws_user = hmmmm.get_mut(&user_ws_id).ok_or("Websocket gone away")?;
    let sub_type = SubType::Leaderboard;
    if let Some(toggle) = data.all{
        sub_all(&sub_type, ws_user, toggle).await;
    }
    if let Some(ids) = data.sub_leaderboard_ids{
        sub(&sub_type, ws_user, ids.iter()).await;
    }
    if let Some(ids) = data.unsub_leaderboard_ids{
        unsub(&sub_type, ws_user, ids.iter()).await;
    }
    let subscription = ws_user.subscriptions.get_ez(&SubType::Leaderboard);
    let data = match subscription.all{
        true => {
            db::latest_leaderboards(&conn, None, None)
        },
        false => {
            db::latest_leaderboards(&conn, None, Some(subscription.ids.iter().collect()))
        }
    }?;
    let resp_msg = WSMsgOut::resp(message_id, method, data);
    serde_json::to_string(&resp_msg).map_err(|e| e.into())
}

pub async fn insert_leaderboards(method: &str, message_id: Uuid, data: Vec<Leaderboard>, conn: PgConn, ws_conns: &mut WSConnections_) -> Result<String, BoxError>{
    let out: Vec<Leaderboard> = insert!(&conn, leaderboards::table, data)?;
    publish::<SubType, Leaderboard>(
        ws_conns, &out, SubType::League, None
    ).await?;
    publish::<SubType, Leaderboard>(
        ws_conns, &out, SubType::Leaderboard, None
    ).await?;
    let resp_msg = WSMsgOut::resp(message_id, method, out);
    serde_json::to_string(&resp_msg).map_err(|e| e.into())
}

pub async fn update_leaderboards(method: &str, message_id: Uuid, data: Vec<LeaderboardUpdate>, conn: PgConn, ws_conns: &mut WSConnections_) -> Result<String, BoxError>{
    let out: Vec<Leaderboard> = conn.build_transaction().run(|| {
        data.iter().map(|c| {
        update!(&conn, leaderboards, leaderboard_id, c)
    }).collect()})?;
    publish::<SubType, Leaderboard>(
        ws_conns, &out, SubType::League, None
    ).await?;
    publish::<SubType, Leaderboard>(
        ws_conns, &out, SubType::Leaderboard, None
    ).await?;
    let resp_msg = WSMsgOut::resp(message_id, method, out);
    serde_json::to_string(&resp_msg).map_err(|e| e.into())
}

pub async fn insert_stats(method: &str, message_id: Uuid, data: Vec<Stat>, conn: PgConn, ws_conns: &mut WSConnections_) -> Result<String, BoxError>{
    let out: Vec<Stat> = insert!(&conn, stats::table, &data)?;
    let to_publish: Vec<ApiLeaderboardLatest> = db::latest_leaderboards(&conn, None, Some(out.iter().map(|x| &x.leaderboard_id).dedup().collect()))?;
    publish::<SubType, ApiLeaderboardLatest>(
        ws_conns, &to_publish, SubType::League, None
    ).await?;
    // publish::<SubType, Stat>(
    //     ws_conns, &out, SubType::League, Some(id_map)
    // ).await?;
    publish::<SubType, ApiLeaderboardLatest>(
        ws_conns, &to_publish, SubType::Leaderboard, None
    ).await?;
    let resp_msg = WSMsgOut::resp(message_id, method, out);
    serde_json::to_string(&resp_msg).map_err(|e| e.into())
}

pub async fn get_latest_leaderboards(method: &str, message_id: Uuid, data: Vec<Uuid>, conn: PgConn, _: &mut WSConnections_) -> Result<String, BoxError>{
    let out: Vec<ApiLeaderboardLatest> = db::latest_leaderboards(&conn, None, Some(data.iter().collect_vec()))?;
    let resp_msg = WSMsgOut::resp(message_id, method, out);
    serde_json::to_string(&resp_msg).map_err(|e| e.into())
}