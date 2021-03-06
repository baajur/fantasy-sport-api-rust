table! {
    leaderboards (leaderboard_id) {
        leaderboard_id -> Uuid,
        league_id -> Uuid,
        name -> Text,
        meta -> Jsonb,
        timespan -> Tstzrange,
    }
}

table! {
    stats (leaderboard_id, player_id, timestamp) {
        player_id -> Uuid,
        leaderboard_id -> Uuid,
        timestamp -> Timestamptz,
        points -> Float4,
        meta -> Jsonb,
    }
}

joinable!(stats -> leaderboards (leaderboard_id));

allow_tables_to_appear_in_same_query!(leaderboards, stats,);
