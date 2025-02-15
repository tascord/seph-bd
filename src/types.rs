use std::marker::PhantomData;

use serde::{Deserialize, Deserializer, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Card {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub sfid: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub colours: Vec<String>,
    #[serde(default)]
    pub type_line: String,
    #[serde(default)]
    pub cmc: usize,
    #[serde(default)]
    pub decks: Vec<String>,
    #[serde(default)]
    pub mainboard_count: usize,
    #[serde(default)]
    pub sideboard_count: usize,
}

pub struct DebugDeser<'de, T: Deserialize<'de> + Clone>(T, PhantomData<&'de T>);
impl<'de, T: Deserialize<'de> + Clone> Deserialize<'de> for DebugDeser<'de, T> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de> {
        
        // log the input as a string without consuming deserializer
        let input = Box::leak(Box::new(String::deserialize(deserializer)?.to_string())) as &'static str;
        println!("Deserializing: {}", input);

        // Deserialize the input into the desired type
        let value = serde_json::from_str(&input).map_err(serde::de::Error::custom)?;
        Ok(DebugDeser(value, PhantomData))

    }
}

impl Into<DeckCard> for Card {
    fn into(self) -> DeckCard {
        DeckCard {
            id: self.id,
            sfid: self.sfid,
            name: self.name,
            colours: self.colours,
            type_line: self.type_line,
            cmc: self.cmc,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DeckCard {
    pub id: String,
    pub sfid: String,
    pub name: String,
    pub colours: Vec<String>,
    pub type_line: String,
    #[serde(deserialize_with = "float_to_int")]
    pub cmc: usize,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Deck {
    pub id: String,
    #[serde(deserialize_with = "bool_from_str")]
    pub currently_legal: bool,
    pub name: String,
    pub url: String,
    pub mainboard: Vec<DeckCard>,
    pub sideboard: Vec<DeckCard>,
    #[serde(deserialize_with = "float_to_int")]
    pub created: usize,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Event {
    pub id: String,
    pub decks: Vec<(String, String)>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Point {
    #[serde(deserialize_with = "float_to_int")]
    pub rating: usize,
    pub cards: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct UsageOverTime {
    pub id: String,
    /// (Date, (Mainboard, Sideboard))
    pub data: Vec<(usize, (usize, usize))>,
}

// Deser string to boolean
pub fn bool_from_str<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: Deserializer<'de>,
{
    let s: &str = Deserialize::deserialize(deserializer)?;
    match s {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(serde::de::Error::custom("expected true or false")),
    }
}

// Deser floating point number to integer
pub fn float_to_int<'de, D>(deserializer: D) -> Result<usize, D::Error>
where
    D: Deserializer<'de>,
{
    let f: f64 = Deserialize::deserialize(deserializer)?;
    Ok(f as usize)
}
