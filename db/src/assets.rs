use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct Event {
    pub uid: String,
    pub name: String,
    pub location: String,
    pub start: f64,
    pub end: f64,
    pub description: String,
    pub professor: Option<String>,
    pub subject: String,
    #[serde(rename = "type")]
    pub event_type: EventType,
}

#[derive(Deserialize, Debug)]
pub enum EventType {
    CM,
    TD,
    TP,
    Projet,
}

impl Event {
    pub fn from_parsed_ical() -> Vec<Self> {
        let source = include_str!("../../assets/cal.json");
        serde_json::from_str(source).unwrap()
    }
}

#[derive(Deserialize)]
pub struct StudentName {
    pub first_name: String,
    pub last_name: String,
}

impl StudentName {
    pub fn from_parsed_json() -> Vec<Self> {
        let source = include_str!("../../assets/students.json");
        serde_json::from_str(source).unwrap()
    }
}
