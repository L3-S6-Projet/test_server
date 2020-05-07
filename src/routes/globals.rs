use chrono::{DateTime, NaiveDateTime, Utc};
use db::{
    models::{Occupancy, OccupancyType},
    Database, LockedDb,
};
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashMap;

#[derive(Serialize)]
pub struct FailureResponse {
    status: &'static str,
    code: ErrorCode,
}

impl FailureResponse {
    pub fn new(code: ErrorCode) -> Self {
        Self {
            status: "error",
            code,
        }
    }

    pub fn new_reply(code: ErrorCode) -> warp::reply::Json {
        warp::reply::json(&Self::new(code))
    }
}

#[derive(Serialize)]
#[allow(dead_code)]
pub enum ErrorCode {
    InvalidCredentials,
    InsufficientAuthorization,
    MalformedData,
    InvalidOldPassword,
    PasswordTooSimple,
    InvalidEmail,
    InvalidPhoneNumber,
    InvalidRank,
    InvalidID,
    InvalidCapacity,
    TeacherInCharge,
    ClassroomUsed,
    InvalidLevel,
    ClassUsed,
    StudentInClass,
    SubjectUsed,
    TeacherNotInCharge,
    LastTeacherInSubject,
    LastGroupInSubject,
    ClassroomAlreadyOccupied,
    ClassOrGroupAlreadyOccupied,
    InvalidOccupancyType,
    EndBeforeStart,
    TeacherDoesNotTeach,
    IllegalOccupancyType,
    Unknown,
    NotFound,
    IllegalRequest,
}

#[derive(Serialize)]
pub struct SimpleSuccessResponse {
    status: &'static str,
}

impl SimpleSuccessResponse {
    pub fn new() -> Self {
        Self { status: "success" }
    }
}

#[derive(Deserialize, Debug)]
pub struct PaginatedQueryableListRequest {
    pub query: Option<String>,
    pub page: Option<usize>,
}

impl PaginatedQueryableListRequest {
    /// Checks that the page number is valid, and if its not it returns 1
    pub fn normalized_page_number(&self) -> usize {
        self.page
            .map(|v| if v >= 1 { v } else { 1 })
            .unwrap_or(1usize)
    }
}

#[derive(Serialize)]
pub struct AccountCreatedResponse<'a> {
    pub status: &'static str,
    pub username: &'a str,
    pub password: &'a str,
}

pub fn deserialize_some<'de, T, D>(deserializer: D) -> Result<Option<T>, D::Error>
where
    T: Deserialize<'de>,
    D: Deserializer<'de>,
{
    Deserialize::deserialize(deserializer).map(Some)
}

#[derive(Deserialize, Debug)]
pub struct OccupanciesRequest {
    pub start: Option<u64>,
    pub end: Option<u64>,
    pub occupancies_per_day: Option<u32>,
}

#[derive(Serialize)]
pub struct OccupanciesListElement<'a> {
    pub id: u32,
    pub classroom_name: Option<&'a str>,
    pub group_name: Option<String>,
    pub subject_name: Option<&'a str>,
    pub teacher_name: String,
    pub start: u64,
    pub end: u64,
    pub occupancy_type: &'a OccupancyType,
    pub class_name: Option<&'a str>,
    pub name: &'a str,
}

#[derive(Serialize)]
pub struct OccupanciesListResponse<'a> {
    status: &'static str,
    days: Vec<OccupanciesListItemResponse<'a>>,
}

#[derive(Serialize)]
pub struct OccupanciesListItemResponse<'a> {
    date: String,
    occupancies: Vec<OccupanciesListElement<'a>>,
}

impl<'a> OccupanciesListResponse<'a> {
    pub fn from_list(
        db: &'a LockedDb,
        occupancies_list: Vec<&'a Occupancy>,
        occupancies_per_day: Option<u32>,
    ) -> Self {
        let mut occupancies: HashMap<String, Vec<OccupanciesListElement>> = HashMap::new();

        for occupancy in occupancies_list {
            let date = NaiveDateTime::from_timestamp(occupancy.start_datetime as i64, 0);
            let datetime: DateTime<Utc> = DateTime::from_utc(date, Utc);
            let key = format!("{}", datetime.format("%d-%m-%Y"));

            let entry = occupancies.entry(key).or_insert(Vec::new());

            let subject = occupancy.subject_id.map(|subject_id| {
                db.subject_get(subject_id)
                    .expect("subject should be a valid reference")
            });

            let class = subject.map(|subject| {
                db.class_get(subject.class_id)
                    .expect("class should be a valid reference")
            });

            let classroom = occupancy.classroom_id.map(|classroom_id| {
                db.classroom_get(classroom_id)
                    .expect("classroom should be a valid reference")
            });

            let teacher = db
                .user_get_teacher_by_id(occupancy.teacher_id)
                .expect("should be a valid reference");

            entry.push(OccupanciesListElement {
                id: occupancy.id,
                classroom_name: classroom.map(|c| c.name.as_str()),
                group_name: occupancy
                    .group_number
                    .map(|group_number| format!("Groupe {}", group_number + 1)),
                subject_name: subject.map(|s| s.name.as_str()),
                teacher_name: teacher.full_name(),
                start: occupancy.start_datetime,
                end: occupancy.end_datetime,
                occupancy_type: &occupancy.occupancy_type,
                class_name: class.map(|c| c.name.as_ref()),
                name: &occupancy.name,
            });
        }

        let occupancies_per_day = occupancies_per_day.unwrap_or(0);

        // For each entry, keep only the top X results
        for days in occupancies.values_mut() {
            days.sort_by_key(|e| e.start);

            if occupancies_per_day > 0 {
                days.truncate(occupancies_per_day as usize);
            }
        }

        // Re-format occupancies
        let days = occupancies
            .into_iter()
            .map(|(date, vec)| OccupanciesListItemResponse {
                date,
                occupancies: vec,
            })
            .collect();

        Self {
            status: "success",
            days,
        }
    }
}
