use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Card {
    pub id: String,
    pub sfid: String,
    pub name: String,
    pub colours: Vec<String>,
    pub type_line: String,
    pub cmc: usize,
    pub decks: Vec<String>,
    pub mainboard_count: usize,
    pub sideboard_count: usize,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Deck {
    pub id: String,
    pub currently_legal: bool,
    pub name: String,
    pub url: String,
    pub mainboard: Vec<Card>,
    pub sideboard: Vec<Card>,
    pub created: usize,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Event {
    pub id: String,
    pub decks: Vec<(String, String)>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Point {
    pub rating: u8,
    pub cards: Vec<String>,
}

