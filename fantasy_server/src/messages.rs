// #[derive(Deserialize)]
// #[serde(tag = "method")]
// pub struct WSReq<'a> {
//     pub message_id: Uuid,
//     pub method: &'a str,
//     // This is left as string, rather than an arbitrary serde_json::Value.
//     // because if you says it's a Value, then do serde_json::from_value on it, and it fails, the error message is really bad
//     // SO want to do a second from_string on the data
//     pub data: serde_json::Value
// }
use uuid::Uuid;
use serde::Deserialize;
use crate::types::{drafts::*, users::*, fantasy_teams::*, leagues::*};

#[derive(Deserialize, Debug)]
pub struct SubUser{
    pub toggle: bool,
}

#[derive(Deserialize, Debug)]
pub struct SubDraft{
    pub sub_draft_ids: Option<Vec<Uuid>>,
    pub unsub_draft_ids: Option<Vec<Uuid>>,
    pub all: Option<bool>
}

#[derive(Deserialize, Debug)]
pub struct SubLeague{
    pub sub_league_ids: Option<Vec<Uuid>>,
    pub unsub_league_ids: Option<Vec<Uuid>>,
    pub all: Option<bool>
}

#[derive(Deserialize)]
#[serde(tag = "method")]
pub enum WSReq {
    League { message_id: Uuid, data: Vec<League>},
    LeagueUpdate { message_id: Uuid, data: Vec<LeagueUpdate>},
    Period { message_id: Uuid, data: Vec<Period>},
    PeriodUpdate { message_id: Uuid, data: Vec<PeriodUpdate>},
    StatMultiplier { message_id: Uuid, data: Vec<StatMultiplier>},
    StatMultiplierUpdate { message_id: Uuid, data: Vec<StatMultiplierUpdate>},
    ExternalUser { message_id: Uuid, data: Vec<ExternalUser>},
    ExternalUserUpdate { message_id: Uuid, data: Vec<ExternalUserUpdate>},
    DraftQueue { message_id: Uuid, data: Vec<DraftQueue>},
    DraftChoiceUpdate { message_id: Uuid, data: Vec<DraftChoiceUpdate>},
    Pick { message_id: Uuid, data: Vec<Pick>},
    PickUpdate { message_id: Uuid, data: Vec<PickUpdate>},
    FantasyTeam { message_id: Uuid, data: Vec<FantasyTeam>},
    FantasyTeamUpdate { message_id: Uuid, data: Vec<FantasyTeamUpdate>},
    SubLeague { message_id: Uuid, data: SubLeague},
    SubDraft { message_id: Uuid, data: SubDraft},
    SubUser { message_id: Uuid, data: SubUser},
}