use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct Classroom {
    pub id: u32,
    pub name: String,
    pub capacity: u16,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct User {
    pub id: u32,
    pub first_name: String,
    pub last_name: String,
    pub username: String,
    pub password: String,
    pub kind: UserKind,
}

impl User {
    pub fn full_name(&self) -> String {
        format!("{} {}", self.first_name, self.last_name)
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub enum UserKind {
    Administrator,
    Teacher(TeacherInformations),
    Student(StudentInformations),
}

impl UserKind {
    pub fn is_administrator(&self) -> bool {
        match self {
            Self::Administrator => true,
            _ => false,
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct TeacherInformations {
    pub phone_number: Option<String>,
    pub email: Option<String>,
    pub rank: Rank,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum Rank {
    #[serde(rename = "MACO")]
    Lecturer,
    #[serde(rename = "PROF")]
    Professor,
    PRAG,
    ATER,
    PAST,
    #[serde(rename = "MONI")]
    Monitor,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct StudentInformations {
    pub class_id: u32,
}

#[derive(Serialize, Deserialize)]
pub struct Class {
    pub id: u32,
    pub name: String,
    pub level: ClassLevel,
}

#[derive(Deserialize, Serialize)]
pub enum ClassLevel {
    L1,
    L2,
    L3,
    M1,
    M2,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct Subject {
    pub id: u32,
    pub class_id: u32,
    pub name: String,
    pub group_count: u32,
}

#[derive(Deserialize, Serialize)]
pub struct SubjectTeacher {
    pub id: u32,
    pub teacher_id: u32,
    pub subject_id: u32,
    pub in_charge: bool,
}

#[derive(Deserialize, Serialize)]
pub struct StudentSubject {
    pub id: u32,
    pub subject_id: u32,
    pub student_id: u32,
    pub group_number: u32,
}

#[derive(Deserialize, Serialize)]
pub struct Occupancy {
    pub id: u32,
    pub classroom_id: Option<u32>,
    pub group_number: Option<u32>,
    pub subject_id: Option<u32>,
    pub teacher_id: u32,
    pub start_datetime: u64,
    pub end_datetime: u64,
    pub occupancy_type: OccupancyType,
    pub name: String,
}

#[derive(Deserialize, Serialize, Clone)]
pub enum OccupancyType {
    CM,
    TD,
    TP,
    #[serde(rename = "PROJ")]
    Projet,
    #[serde(rename = "ADM")]
    Administration,
    #[serde(rename = "EXT")]
    External,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct Modification {
    pub modification_type: ModificationType,
    pub modification_timestamp: u64,
    pub occupancy: ModificationOccupancy,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct ModificationOccupancy {
    pub subject_id: Option<u32>,
    pub class_id: Option<u32>,
    pub occupancy_type: OccupancyType,
    pub occupancy_start: u64,
    pub occupancy_end: u64,
    pub previous_occupancy_start: u64,
    pub previous_occupancy_end: u64,
}

#[derive(Deserialize, Serialize, Clone)]
#[serde(rename_all = "UPPERCASE")]
pub enum ModificationType {
    Create,
    Edit,
    Delete,
}
