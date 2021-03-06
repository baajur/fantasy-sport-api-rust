use serde::{Deserialize, Serialize};
use diesel_utils::{PgConn, DieselTimespan, my_timespan_format};
use crate::schema::*;
use uuid::Uuid;
use serde_json;
use frunk::LabelledGeneric;
use frunk::labelled::transform_from;
use std::collections::HashMap;
use super::{players::*};
use itertools::Itertools;
use crate::diesel::RunQueryDsl;  // imported here so that can run db macros

#[derive(Insertable, Deserialize, LabelledGeneric, Queryable, Serialize, Debug, Identifiable)]
#[table_name = "teams"]
#[primary_key(team_id)]
pub struct Team {
    pub team_id: Uuid,
    pub meta: serde_json::Value,
}


#[derive(Serialize, Deserialize, Debug, LabelledGeneric, Clone)]
pub struct ApiTeam{
    pub team_id: Uuid,
    pub meta: serde_json::Value,
    pub names: Vec<ApiTeamName>,
}

#[derive(Insertable, Queryable, Deserialize, Serialize, Debug, Identifiable, Associations, LabelledGeneric)]
#[primary_key(team_name_id)]
#[belongs_to(Team)]
pub struct TeamName {
    #[serde(skip_serializing)]
    pub team_name_id: Uuid,
    #[serde(skip_serializing)]
    pub team_id: Uuid,
    pub name: String,
    #[serde(with = "my_timespan_format")]
    pub timespan: DieselTimespan,
}

#[derive(Deserialize, Serialize, LabelledGeneric, AsChangeset, Debug)]
#[primary_key(team_id)]
#[table_name = "teams"]
pub struct TeamUpdate {
    pub team_id: Uuid,
    pub meta: Option<serde_json::Value>,
}

#[derive(Deserialize, Serialize, LabelledGeneric, Debug, Clone)]
pub struct ApiTeamName {
    pub name: String,
    #[serde(with = "my_timespan_format")]
    pub timespan: DieselTimespan,
}

#[derive(Deserialize, Insertable, LabelledGeneric, Debug, Clone)]
#[table_name = "team_names"]
pub struct ApiTeamNameNew {
    pub team_id: Uuid,
    pub name: String,
    #[serde(with = "my_timespan_format")]
    pub timespan: DieselTimespan,
}

// #[derive(Serialize, Debug)]
// pub struct ApiTeamsAndPlayers{
//     #[serde(skip_serializing_if = "Option::is_none")]
//     pub teams: Option<Vec<ApiTeam>>,
//     #[serde(skip_serializing_if = "Option::is_none")]
//     pub players: Option<Vec<ApiPlayer>>,
//     #[serde(skip_serializing_if = "Option::is_none")]
//     pub team_players: Option<Vec<TeamPlayer>>
// }

#[derive(Serialize, Deserialize, Debug)]
pub struct ApiTeamWithPlayersHierarchy{
    pub team_id: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub names: Option<Vec<ApiTeamName>>,
    pub meta: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub players: Option<Vec<ApiTeamPlayerOut>>
}

impl ApiTeamWithPlayersHierarchy{
    pub fn from_api_team(teams: Vec<ApiTeam>) -> Vec<Self>{
        teams.into_iter().map(|t|{
            Self{
                team_id: t.team_id,
                names: Some(t.names),
                meta: t.meta,
                players: None
            }
        }).collect_vec()
    }

    pub fn from_team(teams: Vec<Team>) -> Vec<Self>{
        teams.into_iter().map(|t|{
            Self{
                team_id: t.team_id,
                names: None,
                meta: t.meta,
                players: None
            }
        }).collect_vec()
    }
}

#[derive(Queryable, Insertable, Deserialize, Serialize, Debug, Identifiable, Associations)]
#[primary_key(team_player_id)]
pub struct TeamPlayer {
    #[serde(skip_serializing)]
    pub team_player_id: Uuid,
    pub player_id: Uuid,
    pub team_id: Uuid,
    #[serde(with = "my_timespan_format")]
    pub timespan: DieselTimespan,
}

#[derive(Queryable, Insertable, LabelledGeneric, Deserialize, Serialize, Debug, Clone)]
#[table_name = "team_players"]
pub struct ApiTeamPlayer {
    pub player_id: Uuid,
    pub team_id: Uuid,
    #[serde(with = "my_timespan_format")]
    pub timespan: DieselTimespan,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct ApiTeamPlayerOut {
    pub team_id: Uuid,
    pub player: ApiPlayer,
    #[serde(with = "my_timespan_format")]
    pub timespan: DieselTimespan,
}


impl ApiTeam{
    
    pub fn from_rows(rows: Vec<(Team, TeamName)>) -> Vec<Self>{
        // Group rows by team-id using hashmap, build a list of different team names
        // Assume if a team has no names ever, we dont care about it
        let mut acc: HashMap<Uuid, (Team, Vec<ApiTeamName>)> = HashMap::new();
        acc = rows.into_iter().fold(acc, |mut acc, (team, team_name)| {
            let team_name: ApiTeamName = transform_from(team_name);
            match acc.get_mut(&team.team_id) {
                Some(t) => {t.1.push(team_name);},
                None => {acc.insert(team.team_id, (team, vec![team_name]));},
            }
            acc
        });

        acc.into_iter().map(|(team_id, v)|{
            Self{
                team_id: team_id,
                meta: v.0.meta,
                names: v.1
            }
        })
        .collect_vec()
    }

    pub fn insert(conn: PgConn, teams: Vec<Self>) -> Result<bool, diesel::result::Error>{
        let names: Vec<TeamName> = teams.clone().into_iter().flat_map(|t| {
            let team_id = t.team_id;
            t.names.into_iter().map(|n| {
                TeamName{
                    team_name_id: Uuid::new_v4(), team_id, name: n.name, timespan: n.timespan
                }
            }).collect_vec()
        }).collect();
        let raw_teams: Vec<Team> = teams.into_iter().map(transform_from).collect();
        insert_exec!(&conn, teams::table, raw_teams)?;
        insert_exec!(&conn, team_names::table, names)?;
        Ok(true)
    }
}
