use std::collections::HashMap;
use warp_ws_server::{Subscription, Subscriptions};

#[derive(Debug, PartialEq, Eq, Hash)]
pub enum SubType {
    League,
    Leaderboard,
}

pub struct MySubHandler {}

impl warp_ws_server::SubscriptionHandler<SubType> for MySubHandler {
    fn new() -> Subscriptions<SubType> {
        let mut inner = HashMap::new();
        inner.insert(SubType::League, Subscription::new());
        inner.insert(SubType::Leaderboard, Subscription::new());
        inner
    }
}
