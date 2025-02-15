use std::{collections::HashMap, env, ops::Not};

use bson::{doc, Bson, Document};
use futures::{future::{join_all, try_join_all}, StreamExt, TryStreamExt};
use mongodb::{Client, Database};
use serde::{Deserialize, Serialize};
use types::{Card, DebugDeser, Deck, DeckCard, Event, Point, UsageOverTime};

mod types;

#[tokio::main]
async fn main() {
    dotenv::dotenv().expect("Whoopsies");

    let db = Client::with_uri_str(env::var("MONGO").unwrap())
        .await
        .unwrap()
        .database("seph");

    // update_events(&db).await;
    // update_cards(&db).await;
    update_card_usage(&db).await;
}

mod data {
    use futures::StreamExt;

    use super::*;

    pub async fn event_by_id(db: &Database, id: String) -> Event {
        db.collection::<Event>("events")
            .find(doc! { "id": id })
            .await
            .unwrap()
            .deserialize_current()
            .unwrap()
    }

    pub async fn decks_in(db: &Database, ids: Vec<String>) -> Vec<Deck> {
        db.collection::<Deck>("decks")
            .find(doc! { "id": { "$in": ids } })
            .await
            .unwrap()
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .filter_map(|d| d.ok())
            .collect::<Vec<_>>()
    }

    pub async fn points(db: &Database) -> HashMap<usize, Vec<String>> {
        db.collection::<Point>("points")
            .find(doc! {})
            .await
            .unwrap()
            .try_collect::<Vec<_>>()
            .await
            .unwrap()
            .into_iter()
            .map(|p| (p.rating, p.cards))
            .collect::<HashMap<_, _>>()
    }
}

//

async fn update_events(db: &Database) {
    let events = db
        .collection::<Event>("events")
        .find(Document::default())
        .await
        .unwrap()
        .try_collect::<Vec<_>>()
        .await
        .unwrap()
        .into_iter()
        .map(|e| (e, db.clone()))
        .map(|(e, db)| async move {
            let stats = event_stats(db.clone(), e.id.clone()).await;
            match db.collection::<Bson>("stats").find_one(doc! { "type": stats.r#type.clone() }).await.unwrap().is_some() {
                true => {
                    db.collection("stats").replace_one(doc! { "type": stats.r#type.clone() }, stats).await.unwrap();
                }
                false => {
                    db.collection("stats").insert_one(stats).await.unwrap();
                }
            }
        })
        .collect::<Vec<_>>();

    join_all(events).await;
}

async fn update_card_usage(db: &Database) {
    let usage = card_usage_stats(db.clone()).await;
    join_all(usage.into_iter().map(|e| async {
        match db.collection::<Bson>("usage").find_one(doc! { "id": e.id.clone() }).await.unwrap().is_some() {
            true => {
                db.collection::<UsageOverTime>("usage").replace_one(doc! { "type": e.id.clone() }, e).await.unwrap();
            }
            false => {
                db.collection::<UsageOverTime>("usage").insert_one(e).await.unwrap();
            }
        }}
    )).await;
}

async fn update_cards(db: &Database) {
    let stats = card_stats(db.clone()).await;
    match db.collection::<Bson>("stats").find_one(doc! { "type": stats.r#type.clone() }).await.unwrap().is_some() {
        true => {
            db.collection("stats").replace_one(doc! { "type": stats.r#type.clone() }, stats).await.unwrap();
        }
        false => {
            db.collection("stats").insert_one(stats).await.unwrap();
        }
    }
}
//

#[derive(Serialize, Deserialize, Clone, Debug)]
struct Stats {
    r#type: String,
    mainboard_uses: HashMap<String, usize>,
    sideboard_uses: HashMap<String, usize>,
    pointed_uses: HashMap<String, usize>,
    non_pointed_uses: HashMap<String, usize>,
    colour_usage: HashMap<String, usize>,
    cards: HashMap<String, DeckCard>,
}

async fn event_stats(db: Database, id: String) -> Stats {
    let event = data::event_by_id(&db, id).await;
    let ids = event
        .decks
        .iter()
        .map(|v| v.0.to_string())
        .collect::<Vec<_>>();

    let decks = data::decks_in(&db, ids).await;
    let points = data::points(&db).await;
    let cards = {
        decks
            .iter()
            .cloned()
            .flat_map(|mut d| {
                d.mainboard.append(&mut d.sideboard);
                d.mainboard
            })
            .map(|c| (c.id.to_string(), c))
            .collect::<HashMap<_, DeckCard>>()
    };

    let is_pointed = cards
        .iter()
        .filter_map(|c| points.iter().any(|p| p.1.contains(c.0)).then_some(c.0))
        .collect::<Vec<_>>();

    // -- //

    let mainboard_uses = decks
        .iter()
        .flat_map(|d| d.mainboard.iter().map(|c| c.id.to_string()))
        .fold(HashMap::<String, usize>::new(), |mut a, c| {
            a.insert(c.to_string(), a.get(&c).unwrap_or(&0) + 1);
            a
        });

    let sideboard_uses = decks
        .iter()
        .flat_map(|d| d.sideboard.iter().map(|c| c.id.to_string()))
        .fold(HashMap::<String, usize>::new(), |mut a, c| {
            a.insert(c.to_string(), a.get(&c).unwrap_or(&0) + 1);
            a
        });

    let pointed_uses =
        is_pointed
            .iter()
            .cloned()
            .fold(HashMap::<String, usize>::new(), |mut a, c| {
                a.insert(c.to_string(), a.get(c).unwrap_or(&0) + 1);
                a
            });

    let non_pointed_uses = cards
        .iter()
        .filter_map(|c| is_pointed.contains(&c.0).not().then_some(c.0.to_string()))
        .fold(HashMap::<String, usize>::new(), |mut a, c| {
            a.insert(c.to_string(), a.get(&c).unwrap_or(&0) + 1);
            a
        });

    let colour_usage = cards.iter().fold(HashMap::<String, usize>::new(), |mut a, c| {
        c.1.colours.iter().for_each(|col| {
            a.insert(col.to_string(), a.get(col).unwrap_or(&0) + 1);
        });

        a
    });

    println!("Got stats for {}", event.id);
    Stats {
        r#type: format!("event-{}", event.id),
        mainboard_uses,
        sideboard_uses,
        pointed_uses,
        non_pointed_uses,
        colour_usage,
        cards,
    }
}

async fn card_stats(db: Database) -> Stats {
    let cards = db
        .collection::<Card>("cards")
        .find(doc! {})
        .await
        .unwrap()
        .filter_map(|v| async { v.ok() })
        .collect::<Vec<_>>()
        .await
        .into_iter()
        .map(|c| (c.id.to_string(), c))
        .collect::<HashMap<_, Card>>();

    let points = data::points(&db).await;
    let is_pointed = cards
        .iter()
        .filter_map(|c| points.iter().any(|p| p.1.contains(c.0)).then_some(c.0))
        .collect::<Vec<_>>();

    // -- //

    let mainboard_uses = cards.iter().map(|c| (c.0.to_string(), c.1.mainboard_count)).collect::<HashMap<_, _>>();
    let sideboard_uses = cards.iter().map(|c| (c.0.to_string(), c.1.sideboard_count)).collect::<HashMap<_, _>>();
    let pointed_uses = cards.iter().filter(|c| is_pointed.contains(&c.0)).map(|c| (c.0.to_string(), c.1.mainboard_count + c.1.sideboard_count)).collect::<HashMap<_, _>>();
    let non_pointed_uses = cards.iter().filter(|c| is_pointed.contains(&c.0).not()).map(|c| (c.0.to_string(), c.1.mainboard_count + c.1.sideboard_count)).collect::<HashMap<_, _>>();

    let colour_usage = cards.iter().fold(HashMap::<String, usize>::new(), |mut a, c| {
        c.1.colours.iter().for_each(|col| {
            a.insert(col.to_string(), a.get(col).unwrap_or(&0) + 1);
        });

        a
    });

    Stats {
        r#type: "cards".to_string(),
        mainboard_uses,
        sideboard_uses,
        pointed_uses,
        non_pointed_uses,
        colour_usage,
        cards: cards.iter().map(|c| (c.0.to_string(), c.1.clone().into())).collect(),
    }
}

async fn card_usage_stats(db: Database) -> Vec<UsageOverTime> {
    let cards = db
        .collection::<Card>("cards")
        .find(doc! {})
        .await
        .unwrap()
        .collect::<Vec<_>>()
        .await
        .into_iter()
        .filter_map(|v| v.ok())
        .map(|c| (c.id.to_string(), c))
        .collect::<HashMap<_, Card>>();

    let usages = cards.iter().map({
        |c| {
        let value = db.clone();
        async move {
            value.clone().collection::<Deck>("decks").find(doc! { "mainboard.id": c.0.clone() }).await.unwrap().filter_map(|d| async { d.ok()}).map(|d| {
            (
                d.created,
                (
                    d.mainboard.iter().filter(|c2| c2.id == *c.0).count(),
                    d.sideboard.iter().filter(|c2| c2.id == *c.0).count()
                )
            )
            }).collect::<Vec<_>>().await.into_iter().map(move |data| (c.0, data)).collect::<Vec<_>>()
        }
        }
    }).collect::<Vec<_>>();

    let usages = join_all(usages).await.into_iter().flatten().fold(HashMap::<String, Vec<_>>::new(), |mut a, (card, entry)| {
        a.insert(card.to_string(), {
            let mut v = a.get(card).cloned().unwrap_or(Vec::new());
            v.push(entry);
            v.to_vec()
        });

        a
    });

    usages.into_iter().map(|v| UsageOverTime {
        id: v.0,
        data: v.1,
    }).collect()
}