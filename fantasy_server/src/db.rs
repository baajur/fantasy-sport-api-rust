use crate::schema::{self, *};
use crate::types::{drafts::*, fantasy_teams::*, leagues::*, users::*};
use chrono::{DateTime, Utc};
use diesel::dsl::{count, max, not, now};
//use diesel::expression_methods::PgRangeExpressionMethods;
use diesel::pg::expression::dsl::any;
use diesel::pg::upsert::excluded;
use diesel::prelude::*;
use diesel::ExpressionMethods;
use diesel::PgArrayExpressionMethods;
use diesel::RunQueryDsl;
use diesel::{sql_query, sql_types};
use diesel_utils::{DieselTimespan, PgConn};
use itertools::{izip, Itertools};
use std::collections::HashMap;
use uuid::Uuid;
// use diesel_utils::PgConn;
// use warp_ws_server::WSReq;
// use warp_ws_server::BoxError;
//use warp_ws_server::utils::my_timespan_format::DieselTimespan;

// use diesel::{
//     query_dsl::LoadQuery,
//     PgConnection,
//     Insertable,
//     query_builder::InsertStatement,
// };

// //https://www.reddit.com/r/rust/comments/afkuko/porting_go_to_rust_how_to_implement_a_generic/ee2jbfu?utm_source=share&utm_medium=web2x
// pub fn insert<Model, Table, Values>(req: WSReq<'_>, conn: PgConn, table: Table) -> Result<Vec<Model>, BoxError>
// where
//     Model: Serialize + DeserializeOwned,
//     Vec<Model>: Insertable<Table, Values=Values>,
//     InsertStatement<Table, Values>: LoadQuery<PgConnection, Model>
// {
//     let deserialized: Vec<Model> = serde_json::from_value(req.data)?;
//     Ok(diesel::insert_into(table).values(deserialized).get_results(&conn)?)
// }

sql_function!(fn upper(x: sql_types::Range<sql_types::Timestamptz>) -> sql_types::Timestamptz);

pub fn get_full_leagues(
    conn: &PgConnection,
    league_ids_filter: Option<Vec<&Uuid>>,
) -> Result<Vec<ApiLeague>, diesel::result::Error> {
    let leagues: Vec<League> = match league_ids_filter {
        Some(league_ids) => leagues::table
            .filter(leagues::dsl::league_id.eq(any(league_ids)))
            .load::<League>(conn),
        None => leagues::table.load(conn),
    }?;
    let periods = Period::belonging_to(&leagues).load::<Period>(conn)?;
    let stats = StatMultiplier::belonging_to(&leagues).load::<StatMultiplier>(conn)?;
    let max_players_per_position =
        MaxPlayersPerPosition::belonging_to(&leagues).load::<MaxPlayersPerPosition>(conn)?;
    let fantasy_teams = FantasyTeam::belonging_to(&leagues).load::<FantasyTeam>(conn)?;
    let grouped_periods = periods.grouped_by(&leagues);
    let grouped_stats = stats.grouped_by(&leagues);
    let grouped_max_players_per_position = max_players_per_position.grouped_by(&leagues);
    let grouped_fantasy_teams = fantasy_teams.grouped_by(&leagues);
    Ok(ApiLeague::from_rows(
        izip!(
            leagues,
            grouped_periods,
            grouped_stats,
            grouped_max_players_per_position,
            grouped_fantasy_teams
        )
        .collect(),
    ))
}

pub fn get_users(
    conn: &PgConnection,
) -> Result<(Vec<ExternalUser>, Vec<Commissioner>), diesel::result::Error> {
    // TODO include what leagues user has team in
    let external_users = external_users::table.load::<ExternalUser>(conn)?;
    let commissioners = commissioners::table.load::<Commissioner>(conn)?;
    Ok((external_users, commissioners))
}

// pub fn get_draft_ids_for_picks(
//     conn: &PgConnection,
//     pick_ids: &Vec<Uuid>,
// ) -> Result<Vec<(Uuid, Uuid)>, diesel::result::Error> {
//     picks::table
//         // important to inner_join between draft-choices and team-drafts (cant do innerjoin().innerjoin(), as that tries joining picks)
//         .inner_join(draft_choices::table.inner_join(team_drafts::table))
//         .select((picks::pick_id, team_drafts::draft_id))
//         .filter(picks::dsl::pick_id.eq(any(pick_ids)))
//         .load(conn)
// }

pub fn get_undrafted_periods(conn: PgConn) -> Result<Vec<Period>, diesel::result::Error> {
    periods::table
        .select(periods::all_columns)
        .left_join(drafts::table)
        .filter(drafts::draft_id.is_null())
        .order(periods::draft_lockdown)
        .load::<Period>(&conn)
    //.first::<Period>(conn)
}

pub fn get_valid_picks(
    conn: &PgConnection,
    draft_id: &Uuid,
    period_id: &Uuid,
) -> Result<Vec<Uuid>, diesel::result::Error> {
    valid_players::table
        .select(valid_players::player_id)
        .filter(valid_players::period_id.eq(period_id))
        .filter(not(valid_players::player_id.eq(any(picks::table
            .select(picks::player_id)
            .inner_join(draft_choices::table.inner_join(team_drafts::table))
            .filter(team_drafts::draft_id.eq(draft_id))))))
        .load(conn)
}

pub fn get_unchosen_draft_choices(
    conn: &PgConn,
) -> Result<Vec<(DraftChoice, Period, TeamDraft, League)>, diesel::result::Error> {
    // So this would join every row, including old rows, then filter most of them out.
    // Should check postgresql optimises nicely.

    // TODO this is way too heavyweight for being called every draft-choice
    // really once draft is fixed, the max_per_blah settings shouldnt be changing. Same for period timespan.
    // When fantasy-teams/users are locked in for draft, then settings should lock as well, and be pulled into memory
    draft_choices::table
        .left_join(picks::table)
        .filter(picks::pick_id.is_null())
        .inner_join(
            team_drafts::table
                .inner_join(drafts::table.inner_join(periods::table.inner_join(leagues::table))),
        )
        .select((
            draft_choices::all_columns,
            periods::all_columns,
            team_drafts::all_columns,
            leagues::all_columns,
        ))
        .order(upper(draft_choices::timespan))
        .load(conn)
}

pub fn get_max_per_position(
    conn: &PgConn,
    league_id: Uuid,
) -> Result<Vec<MaxPlayersPerPosition>, diesel::result::Error> {
    max_players_per_positions::table
        .filter(max_players_per_positions::league_id.eq(league_id))
        .load(conn)
}

pub fn get_randomised_teams_for_league(
    conn: &PgConnection,
    league_id: Uuid,
) -> Result<Vec<FantasyTeam>, diesel::result::Error> {
    // Whilst order by random is expensive on huge tables, I think will only have small amount teams per league so should be fine. finger-cross
    no_arg_sql_function!(
        random,
        sql_types::Integer,
        "Represents the SQL RANDOM() function"
    );

    fantasy_teams::table
        .filter(schema::fantasy_teams::league_id.eq(league_id))
        .order(random)
        .load(conn)
}

pub fn get_league_squad_size(
    conn: &PgConnection,
    league_id: Uuid,
) -> Result<i32, diesel::result::Error> {
    schema::leagues::table
        .select(schema::leagues::squad_size)
        .filter(schema::leagues::league_id.eq(league_id))
        .get_result(conn)
}

pub fn get_draft_queue_for_choice(
    conn: &PgConnection,
    unchosen: DraftChoice,
) -> Result<Option<Vec<Uuid>>, diesel::result::Error> {
    // maybe no queue been upserted. could be empty, could be missing?
    schema::team_drafts::table
        .inner_join(schema::fantasy_teams::table.inner_join(schema::draft_queues::table))
        .inner_join(schema::draft_choices::table)
        .filter(schema::team_drafts::team_draft_id.eq(unchosen.team_draft_id))
        .select(schema::draft_queues::player_ids)
        .get_result(conn)
        .optional()
}

pub fn get_current_players(
    conn: &PgConnection,
    fantasy_team_id: Uuid,
    period_id: Uuid,
) -> Result<Vec<Uuid>, diesel::result::Error> {
    picks::table
        .select(picks::player_id)
        .filter(picks::fantasy_team_id.eq(fantasy_team_id))
        .inner_join(draft_choices::table.inner_join(team_drafts::table.inner_join(drafts::table)))
        .filter(drafts::period_id.eq(period_id))
        .load(conn)
}

pub fn upsert_active_picks(
    conn: &PgConnection,
    data: &Vec<ActivePick>,
) -> Result<Vec<ActivePick>, diesel::result::Error> {
    diesel::insert_into(active_picks::table)
        .values(data)
        // constraint unique pick-with timespan (want same on pick itself so only 1 of player in squad)
        //.on_conflict((active_picks::pick_id, active_picks::timespan))
        .on_conflict(active_picks::active_pick_id)
        .do_update()
        .set(active_picks::timespan.eq(excluded(active_picks::timespan)))
        //.set(name.eq(coalesce::<sql_types::Text>(excluded(name), name)))
        .get_results(conn)
}

#[derive(QueryableByName, Debug)]
pub struct VecUuid {
    #[sql_type = "sql_types::Array<sql_types::Uuid>"]
    pub inner: Vec<Uuid>,
}

pub fn get_all_updated_teams(
    conn: &PgConnection,
    ids: &Vec<Uuid>,
) -> Result<Vec<VecUuid>, diesel::result::Error> {
    // https://www.reddit.com/r/PostgreSQL/comments/gjsham/query_to_list_combinations_of_band_members/
    let sql = "
        WITH upper_and_lower AS (select t1.timespan,
        (select array_agg(t2.pick_id) 
         from picks t2
         where t2.timespan @> lower(t1.timespan)) as lower_ids,
        (select array_agg(t3.pick_id) 
         from picks t3
         where t3.timespan @> upper(t1.timespan)) as upper_ids
        from (
            select timespan
            from picks where pick_id = ANY($1)
        ) as t1)

        select distinct ids as inner from 
        (select lower_ids as ids, lower(timespan) as ttime from upper_and_lower 
        union 
            select upper_ids as ids, upper(timespan) as ttime from upper_and_lower
        ) as final_sub;
    ";
    sql_query(sql)
        .bind::<sql_types::Array<sql_types::Uuid>, _>(ids)
        .load::<VecUuid>(conn)
}
pub fn get_all_updated_teams_player_ids(
    conn: &PgConnection,
    ids: &Vec<Uuid>,
) -> Result<Vec<VecUuid>, diesel::result::Error> {
    // https://www.reddit.com/r/PostgreSQL/comments/gjsham/query_to_list_combinations_of_band_members/
    println!("in get_all_updated_teams_player_ids");
    let picks: Vec<Pick> = picks::table
        .filter(picks::pick_id.eq(any(ids)))
        .load(conn)?;
    picks
        .iter()
        .for_each(|x| println!("{}, {:?}", x.pick_id, x.timespan));
    ids.iter().for_each(|x| println!("{}", x));
    let sql = "
        WITH upper_and_lower AS (select t1.timespan,
        (select array_agg(t2.player_id) 
         from picks t2
         where t2.timespan @> lower(t1.timespan)) as lower_ids,
        (select array_agg(t3.player_id) 
         from picks t3
         where t3.timespan @> upper(t1.timespan)) as upper_ids
        from (
            select timespan
            from picks where pick_id = ANY($1)
        ) as t1)

        select distinct ids as inner from 
        (select lower_ids as ids, lower(timespan) as ttime from upper_and_lower 
        union 
            select upper_ids as ids, upper(timespan) as ttime from upper_and_lower
        ) as final_sub;
    ";
    let out = sql_query(sql)
        .bind::<sql_types::Array<sql_types::Uuid>, _>(ids)
        .load::<VecUuid>(conn);
    println!("get_all_updated_teams_player_ids: {:?}", out);
    out
}

pub fn get_singular_updated_teams_player_ids(
    conn: &PgConnection,
    fantasy_team_id: &Uuid,
    timespan: &DieselTimespan,
) -> Result<Vec<Uuid>, diesel::result::Error> {
    // https://www.reddit.com/r/PostgreSQL/comments/gjsham/query_to_list_combinations_of_band_members/
    println!("in get_singular_updated_teams_player_ids");
    let out = picks::table
        .select(picks::player_id)
        .filter(picks::fantasy_team_id.eq(fantasy_team_id))
        .filter(picks::timespan.eq(timespan))
        .load(conn)?;
    println!("singular_updated_teams_player_ids {}", out.len());
    println!("singular_updated_teams_player_ids: {:?}", out);
    Ok(out)
}

pub fn get_leagues_for_picks(
    conn: &PgConnection,
    pick_ids: &Vec<Uuid>,
) -> Result<Vec<League>, diesel::result::Error> {
    picks::table
        .inner_join(fantasy_teams::table.inner_join(leagues::table))
        .filter(picks::pick_id.eq(any(pick_ids)))
        .select(leagues::all_columns)
        .load(conn)
}

pub fn get_drafts_for_picks(
    conn: &PgConnection,
    pick_ids: Vec<Uuid>,
) -> Result<Vec<ApiDraft>, diesel::result::Error> {
    // kill me now.
    let picks: Vec<Pick> = picks::table
        .filter(picks::pick_id.eq(any(pick_ids)))
        .load(conn)?;
    let fantasy_team_ids = picks.iter().map(|p| p.fantasy_team_id).collect_vec();
    let fantasy_teams: Vec<FantasyTeam> = fantasy_teams::table
        .filter(fantasy_teams::fantasy_team_id.eq(any(fantasy_team_ids)))
        .load(conn)?;
    let team_drafts: Vec<TeamDraft> = TeamDraft::belonging_to(&fantasy_teams).load(conn)?;
    let draft_choices: Vec<DraftChoice> = DraftChoice::belonging_to(&team_drafts).load(conn)?;
    let active_picks: Vec<ActivePick> = ActivePick::belonging_to(&picks).load(conn)?;
    let draft_ids = team_drafts.iter().map(|x| x.draft_id).collect_vec();
    let drafts: Vec<Draft> = drafts::table
        .filter(drafts::draft_id.eq(any(draft_ids)))
        .load(conn)?;

    let grouped_active_picks = active_picks.grouped_by(&picks);
    let pick_level: Vec<(Pick, Vec<ActivePick>)> =
        picks.into_iter().zip(grouped_active_picks).collect_vec();
    let grouped_picks = pick_level.grouped_by(&fantasy_teams);
    let draft_choices_and_picks: Vec<(DraftChoice, Vec<(Pick, Vec<ActivePick>)>)> =
        draft_choices.into_iter().zip(grouped_picks).collect();
    let grouped_draft_choices = draft_choices_and_picks.grouped_by(&team_drafts);
    let team_drafts_level: Vec<(TeamDraft, Vec<(DraftChoice, Vec<(Pick, Vec<ActivePick>)>)>)> =
        team_drafts.into_iter().zip(grouped_draft_choices).collect();
    let grouped_drafts = team_drafts_level.grouped_by(&drafts);
    let draft_level: Vec<(
        Draft,
        Vec<(TeamDraft, Vec<(DraftChoice, Vec<(Pick, Vec<ActivePick>)>)>)>,
    )> = drafts.into_iter().zip(grouped_drafts).collect();
    let fantasy_team_map: HashMap<Uuid, FantasyTeam> = fantasy_teams
        .into_iter()
        .map(|ft| (ft.fantasy_team_id, ft))
        .collect();
    let out: Vec<ApiDraft> = draft_level
        .into_iter()
        .map(|(d, v)| {
            let mut league_id: Option<Uuid> = None;
            let team_drafts = v
                .into_iter()
                .map(|(td, v)| {
                    let mut total_active_picks: Vec<ApiPick> = vec![];
                    let ft = fantasy_team_map.get(&td.fantasy_team_id).unwrap();
                    league_id = Some(ft.league_id);
                    let draft_choices = v
                        .into_iter()
                        .map(|(dc, picks)| {
                            let pick_opt = picks.into_iter().nth(0);
                            if let Some((pick, active_picks)) = pick_opt {
                                total_active_picks.extend(
                                    active_picks
                                        .iter()
                                        .map(|ap| ApiPick {
                                            pick_id: ap.pick_id,
                                            player_id: pick.player_id,
                                            timespan: ap.timespan,
                                            draft_id: None,
                                            fantasy_team_id: None,
                                        })
                                        .collect_vec(),
                                );
                                ApiDraftChoice2 {
                                    draft_choice_id: dc.draft_choice_id,
                                    timespan: dc.timespan,
                                    pick: Some(pick),
                                }
                            } else {
                                ApiDraftChoice2 {
                                    draft_choice_id: dc.draft_choice_id,
                                    timespan: dc.timespan,
                                    pick: None,
                                }
                            }
                        })
                        .collect_vec();
                    ApiTeamDraft {
                        team_draft_id: td.team_draft_id,
                        fantasy_team_id: ft.fantasy_team_id,
                        name: ft.name.clone(),
                        external_user_id: ft.external_user_id,
                        meta: ft.meta.clone(),
                        draft_choices: Some(draft_choices),
                        active_picks: Some(total_active_picks),
                    }
                })
                .collect_vec();
            // I believe it's impossible that team_drafts is empty if we have valid-pick ids
            // guess could pass invalid pick-ids as bug?
            ApiDraft {
                league_id: league_id.unwrap(),
                draft_id: d.draft_id,
                period_id: d.period_id,
                meta: d.meta,
                team_drafts: Some(team_drafts),
            }
        })
        .collect_vec();
    Ok(out)
}

pub fn get_full_drafts(
    conn: &PgConn,
    draft_ids_filt: Option<Vec<&Uuid>>,
) -> Result<Vec<ApiDraft>, diesel::result::Error> {
    // kill me again.
    let drafts: Vec<Draft> = match draft_ids_filt {
        Some(draft_ids) => drafts::table
            .filter(drafts::draft_id.eq(any(draft_ids)))
            .load(conn)?,
        None => drafts::table.load(conn)?,
    };
    let team_drafts: Vec<TeamDraft> = TeamDraft::belonging_to(&drafts).load(conn)?;
    let fantasy_team_ids = team_drafts
        .iter()
        .map(|td| td.fantasy_team_id)
        .collect_vec();
    let fantasy_teams: Vec<FantasyTeam> = fantasy_teams::table
        .filter(fantasy_teams::fantasy_team_id.eq(any(fantasy_team_ids)))
        .load(conn)?;
    let draft_choices: Vec<DraftChoice> = DraftChoice::belonging_to(&team_drafts).load(conn)?;
    let picks: Vec<Pick> = Pick::belonging_to(&draft_choices).load(conn)?;
    let active_picks: Vec<ActivePick> = ActivePick::belonging_to(&picks).load(conn)?;
    let draft_ids = team_drafts.iter().map(|x| x.draft_id).collect_vec();
    let drafts: Vec<Draft> = drafts::table
        .filter(drafts::draft_id.eq(any(draft_ids)))
        .load(conn)?;

    let grouped_active_picks = active_picks.grouped_by(&picks);
    let pick_level: Vec<(Pick, Vec<ActivePick>)> =
        picks.into_iter().zip(grouped_active_picks).collect_vec();
    let grouped_picks = pick_level.grouped_by(&draft_choices);
    let draft_choices_and_picks: Vec<(DraftChoice, Vec<(Pick, Vec<ActivePick>)>)> =
        draft_choices.into_iter().zip(grouped_picks).collect();
    let grouped_draft_choices = draft_choices_and_picks.grouped_by(&team_drafts);
    let team_drafts_level: Vec<(TeamDraft, Vec<(DraftChoice, Vec<(Pick, Vec<ActivePick>)>)>)> =
        team_drafts.into_iter().zip(grouped_draft_choices).collect();
    let grouped_drafts = team_drafts_level.grouped_by(&drafts);
    let draft_level: Vec<(
        Draft,
        Vec<(TeamDraft, Vec<(DraftChoice, Vec<(Pick, Vec<ActivePick>)>)>)>,
    )> = drafts.into_iter().zip(grouped_drafts).collect();
    let fantasy_team_map: HashMap<Uuid, FantasyTeam> = fantasy_teams
        .into_iter()
        .map(|ft| (ft.fantasy_team_id, ft))
        .collect();
    let out: Vec<ApiDraft> = draft_level
        .into_iter()
        .map(|(d, v)| {
            let mut league_id: Option<Uuid> = None;
            let team_drafts = v
                .into_iter()
                .map(|(td, v)| {
                    let mut total_active_picks: Vec<ApiPick> = vec![];
                    let ft = fantasy_team_map.get(&td.fantasy_team_id).unwrap();
                    league_id = Some(ft.league_id);
                    let draft_choices = v
                        .into_iter()
                        .map(|(dc, picks)| {
                            let pick_opt = picks.into_iter().nth(0);
                            if let Some((pick, active_picks)) = pick_opt {
                                total_active_picks.extend(
                                    active_picks
                                        .iter()
                                        .map(|ap| ApiPick {
                                            pick_id: ap.pick_id,
                                            player_id: pick.player_id,
                                            timespan: ap.timespan,
                                            draft_id: None,
                                            fantasy_team_id: None,
                                        })
                                        .collect_vec(),
                                );
                                ApiDraftChoice2 {
                                    draft_choice_id: dc.draft_choice_id,
                                    timespan: dc.timespan,
                                    pick: Some(pick),
                                }
                            } else {
                                ApiDraftChoice2 {
                                    draft_choice_id: dc.draft_choice_id,
                                    timespan: dc.timespan,
                                    pick: None,
                                }
                            }
                        })
                        .collect_vec();
                    ApiTeamDraft {
                        team_draft_id: td.team_draft_id,
                        fantasy_team_id: ft.fantasy_team_id,
                        name: ft.name.clone(),
                        external_user_id: ft.external_user_id,
                        meta: ft.meta.clone(),
                        draft_choices: Some(draft_choices),
                        active_picks: Some(total_active_picks),
                    }
                })
                .collect_vec();
            ApiDraft {
                league_id: league_id.unwrap(),
                draft_id: d.draft_id,
                period_id: d.period_id,
                meta: d.meta,
                team_drafts: Some(team_drafts),
            }
        })
        .collect_vec();
    Ok(out)
}

#[derive(QueryableByName)]
pub struct Count {
    #[sql_type = "sql_types::BigInt"]
    pub inner: i64,
}

#[derive(QueryableByName)]
pub struct Exists {
    #[sql_type = "sql_types::Bool"]
    pub inner: bool,
}

pub fn num_invalid_timed_picks(
    conn: &PgConnection,
    draft_choice_ids: &Vec<Uuid>,
) -> Result<i64, diesel::result::Error> {
    let sql = "select count(*) as inner from draft_choices WHERE draft_choice_id = ANY($1) AND NOT timespan @> now();";
    sql_query(sql)
        .bind::<sql_types::Array<sql_types::Uuid>, _>(draft_choice_ids)
        .get_result::<Count>(conn)
        .map(|x| x.inner)
    // diesel_infix_operator!(Contains, " @> ");

    // use diesel::expression::AsExpression;

    // // Normally you would put this on a trait instead
    // fn contains<T, U>(left: T, right: U) -> Contains<T, U>
    // where
    //     T: Expression,
    //     U: Expression,
    // {
    //     Contains::new(left, right)
    // }

    // draft_choices::table
    //     .count()
    //     .filter(draft_choices::draft_choice_id.eq(any(draft_choice_ids)))
    //     //.filter(not(contains(draft_choices::timespan, now())))
    //     .filter(not(contains(draft_choices::timespan, Utc::now())))
    //     .get_result(conn)
}

pub fn valid_timed_picks_from_team(
    conn: &PgConnection,
    fantasy_team_ids: &Vec<Uuid>,
) -> Result<bool, diesel::result::Error> {
    let sql = "select count(*) as inner from draft_choices JOIN team_drafts USING(team_draft_id) WHERE fantasy_team_id = ANY($1) AND timespan @> now();";
    sql_query(sql)
        .bind::<sql_types::Array<sql_types::Uuid>, _>(fantasy_team_ids)
        .get_result::<Count>(conn)
        // each team should have one draft-choice where timespan contains now
        .map(|x| x.inner == fantasy_team_ids.len() as i64)
}

#[derive(QueryableByName)]
pub struct UuidWrapper {
    #[sql_type = "sql_types::Uuid"]
    pub inner: Uuid,
}

pub fn get_current_draft_choice_id(
    conn: &PgConnection,
    fantasy_team_id: &Uuid,
) -> Result<Uuid, diesel::result::Error> {
    let sql = "select draft_choices.draft_choice_id as inner from draft_choices 
        LEFT JOIN picks using(draft_choice_id) 
        JOIN team_drafts USING(team_draft_id) WHERE team_drafts.fantasy_team_id = $1 AND draft_choices.timespan @> now() AND pick_id IS null;";
    sql_query(sql)
        .bind::<sql_types::Uuid, _>(fantasy_team_id)
        // should diesel error if there's no matches
        .get_result::<UuidWrapper>(conn)
        .map(|x| {println!("get_current_draft_choice_id: {}", x.inner); x.inner})
}

pub fn get_period_from_draft(
    conn: &PgConnection,
    draft_id: &Uuid,
) -> Result<Period, diesel::result::Error> {
    periods::table
        .select(periods::all_columns)
        .inner_join(drafts::table)
        .filter(drafts::draft_id.eq(draft_id))
        .first(conn)
}

// #[derive(QueryableByName)]
// pub struct{
//     #[sql_type = "sql_types::Timestamptz"]
//     pub inner: Uuid,
// }

// TODO restrict to specific league
pub fn get_latest_teams(
    conn: &PgConnection,
) -> Result<HashMap<Uuid, Vec<Uuid>>, diesel::result::Error> {
    let max_timespan: Option<DateTime<Utc>> = picks::table
        .select(max(upper(picks::timespan)))
        .first(conn)?;
    match max_timespan {
        None => Ok(HashMap::new()),
        Some(maxt) => {
            let rows: Vec<(Uuid, Uuid)> = team_drafts::table
                .inner_join(draft_choices::table.inner_join(picks::table))
                .select((team_drafts::fantasy_team_id, picks::player_id))
                .filter(upper(picks::timespan).eq(maxt))
                .load(conn)?;
            Ok(rows
                .into_iter()
                .fold(HashMap::new(), |mut hm, (team_id, player_id)| {
                    match hm.get_mut(&team_id) {
                        Some(v) => {
                            v.push(player_id);
                        }
                        None => {
                            hm.insert(team_id, vec![player_id]);
                        }
                    };
                    hm
                }))
        }
    }
}
