use std::{collections::HashMap, env, ops::Not};

use bson::{doc, Document};
use futures::{future::join_all, StreamExt, TryStreamExt};
use mongodb::{Client, Database};
use serde::{Deserialize, Serialize};
use tokio::{fs::OpenOptions, io::AsyncWriteExt, task::JoinSet};
use types::{Card, Deck, Event, Point};

mod types;

#[tokio::main]
async fn main() {
    dotenv::dotenv().expect("Whoopsies");

    let db = Client::with_uri_str(env::var("MONGO").unwrap())
        .await
        .unwrap()
        .database("seph");

    update_events(&db).await;
}

mod data {
    use super::*;

    pub async fn event_by_id(db: &Database, id: String) -> Event {
        db.collection::<Event>("Event")
            .find(doc! { "id": id })
            .await
            .unwrap()
            .deserialize_current()
            .unwrap()
    }

    pub async fn decks_in(db: &Database, ids: Vec<String>) -> Vec<Deck> {
        db.collection::<Deck>("Decks")
            .find(doc! { "id": { "$in": ids } })
            .await
            .unwrap()
            .try_collect::<Vec<_>>()
            .await
            .unwrap()
    }

    pub async fn points(db: &Database) -> HashMap<u8, Vec<String>> {
        db.collection::<Point>("Points")
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
    dbg!("Update events");

    let events = db
        .collection::<Event>("event")
        .find(Document::default())
        .await
        .unwrap()
        .try_collect::<Vec<_>>()
        .await
        .unwrap()
        .into_iter()
        .map(|e| (e, db.clone()))
        .map(|(e, db)| async move {
            dbg!(&e);
            let stats = event_stats(db, e.id.clone()).await;
            let mut binding = OpenOptions::new();
            let mut file = binding
                .create_new(true)
                .write(true)
                .open(format!("./data/event-{}.json", e.id.replace(" ", "")))
                .await
                .unwrap();

            let data = serde_json::to_string(&stats).unwrap();
            file.write_all(data.as_bytes()).await.unwrap();
        })
        .collect::<Vec<_>>();

    join_all(events).await;
}

//

#[derive(Serialize, Deserialize, Clone, Debug)]
struct EventStats {
    mainboard_uses: HashMap<String, usize>,
    sideboard_uses: HashMap<String, usize>,
    pointed_uses: HashMap<String, usize>,
    non_pointed_uses: HashMap<String, usize>,
}

async fn event_stats(db: Database, id: String) -> EventStats {
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
            .collect::<HashMap<_, Card>>()
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

    println!("Got stats for {}", event.id);
    EventStats {
        mainboard_uses,
        sideboard_uses,
        pointed_uses,
        non_pointed_uses,
    }
}
