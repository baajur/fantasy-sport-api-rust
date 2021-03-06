use serde::{Deserialize, Serialize};
use diesel_utils::{PgConn, DieselTimespan, my_timespan_format, my_timespan_format_opt};
use crate::schema::*;
use uuid::Uuid;
use serde_json;
use frunk::LabelledGeneric;
use super::{competitions::*, matches::*, results::*};
use crate::diesel::RunQueryDsl;  // imported here so that can run db macros
use frunk::labelled::transform_from;
use itertools::{izip, Itertools};
use diesel::prelude::*;


#[derive(Queryable, Serialize, Deserialize, Insertable, Debug, Identifiable, Associations, LabelledGeneric)]
#[belongs_to(Competition)]
#[primary_key(series_id)]
#[table_name = "series"]
pub struct Series {
    pub series_id: Uuid,
    pub name: String,
    pub competition_id: Uuid,
    pub meta: serde_json::Value,
    #[serde(with = "my_timespan_format")]
    pub timespan: DieselTimespan,
}


#[derive(Deserialize, LabelledGeneric, Debug, AsChangeset)]
#[primary_key(series_id)]
#[table_name = "series"]
pub struct SeriesUpdate {
    pub series_id: Uuid,
    pub competition_id: Option<Uuid>,
    pub name: Option<String>,
    pub meta: Option<serde_json::Value>,
    #[serde(with = "my_timespan_format_opt")]
    pub timespan: Option<DieselTimespan>,
}

#[derive(Deserialize, Serialize, Debug, Clone, LabelledGeneric)]
pub struct ApiSeries{
    pub series_id: Uuid,
    pub name: String,
    pub meta: serde_json::Value,
    #[serde(with = "my_timespan_format")]
    pub timespan: DieselTimespan,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub matches: Option<Vec<ApiMatch>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub team_results: Option<Vec<TeamSeriesResult>>,
}

impl ApiSeries{
    pub fn insertable(self, competition_id: Uuid) -> (Series, Vec<Match>, Vec<PlayerResult>, Vec<TeamMatchResult>, Vec<TeamSeriesResult>){
        let series_id = self.series_id;
        let (matches, player_results, team_match_results) = match self.matches{
            Some(matches) => {
                let (mut player_results, mut team_match_results) = (vec![], vec![]);
                let matches = matches
                    .into_iter().map(|m| {
                        let (new_m, mut new_pr, mut new_tr) = m.insertable(series_id);
                        team_match_results.append(&mut new_tr);
                        player_results.append(&mut new_pr);
                        new_m
                    }).collect_vec();
                (matches, player_results, team_match_results)
            },
            None => (vec![], vec![], vec![])
        };
        (
            Series{series_id: self.series_id, name: self.name, meta: self.meta, timespan: self.timespan, competition_id},
            matches, player_results, team_match_results, self.team_results.unwrap_or(vec![])
        )
    }
}

#[derive(Deserialize, Serialize, LabelledGeneric, Debug, Clone)]
pub struct ApiSeriesNew{
    pub series_id: Uuid,
    pub competition_id: Uuid,
    pub name: String,
    pub meta: serde_json::Value,
    #[serde(with = "my_timespan_format")]
    pub timespan: DieselTimespan,
    pub matches: Option<Vec<ApiMatch>>,
    pub team_results: Option<Vec<TeamSeriesResult>>,
}

impl ApiSeriesNew{
    /*
        pub fn insert(conn: &PgConn, new: Vec<ApiMatchNew>) -> Result<Vec<(Match, Vec<PlayerResult>, Vec<TeamMatchResult>)>, diesel::result::Error>{
        let (mut player_results, mut team_match_results) = (vec![], vec![]);
        let matches: Vec<Match> = new
            .into_iter().map(|m|{
                let series_id = m.series_id;
                let m2: ApiMatch = transform_from(m);
                let mut tup = m2.insertable(series_id);
                player_results.append(&mut tup.1);
                team_match_results.append(&mut tup.2);
                tup.0
            }).collect_vec();
        let _: Vec<Match> = insert!(conn, matches::table, &matches)?;
        let _: Vec<PlayerResult> = insert!(conn, player_results::table, &player_results)?;
        let _: Vec<TeamMatchResult> = insert!(conn, team_match_results::table, &team_match_results)?;
        let grouped_presults = player_results.grouped_by(&matches);
        let grouped_tresults = team_match_results.grouped_by(&matches);
        let out = izip!(matches, grouped_presults, grouped_tresults).collect();
        Ok(out)
    }

    */


    pub fn insert(conn: &PgConn, new: Vec<ApiSeriesNew>) -> Result<Vec<
    (Series, Vec<TeamSeriesResult>, Vec<(Match, Vec<PlayerResult>, Vec<TeamMatchResult>)>)
    >, diesel::result::Error>{
        // TODO EWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWW
        // I think i need to define my own iterator so flatmap can flatmap nicely?
        let(
            mut matches, mut player_results, mut team_match_results,
            mut team_results
        ) = (vec![], vec![], vec![], vec![]);
        let series = new
            .into_iter().map(|s|{
                let comp_id = s.competition_id;
                let s2: ApiSeries = transform_from(s);
                let mut tup = s2.insertable(comp_id);
                matches.append(&mut tup.1);
                player_results.append(&mut tup.2);
                team_match_results.append(&mut tup.3);
                team_results.append(&mut tup.4);
                tup.0
            }).collect_vec();
        insert_exec!(conn, series::table, &series)?;
        insert_exec!(conn, matches::table, &matches)?;
        insert_exec!(conn, player_results::table, &player_results)?;
        insert_exec!(conn, team_match_results::table, &team_match_results)?;
        insert_exec!(conn, team_series_results::table, &team_results)?;
        let grouped_presults = player_results.grouped_by(&matches);
        let grouped_tresults = team_match_results.grouped_by(&matches);
        let grouped_series_res = team_results.grouped_by(&series);
        let matches_stuff: Vec<(Match, Vec<PlayerResult>, Vec<TeamMatchResult>)> = izip!(matches, grouped_presults, grouped_tresults).collect();
        let grouped_matches = matches_stuff.grouped_by(&series);
        let out = izip!(series, grouped_series_res, grouped_matches).collect();
        Ok(out)
    }
}

// #[derive(Insertable, Deserialize, Queryable, Serialize, Debug, Identifiable, Associations, Clone)]
// #[primary_key(series_id, team_id)]
// #[belongs_to(Series)]
// #[table_name = "series_teams"]
// pub struct SeriesTeam {
//     series_id: Uuid,
//     pub team_id: Uuid,
// }