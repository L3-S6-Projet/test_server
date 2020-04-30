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
    pub phone_number: String,
    pub email: String,
    pub rank: Rank,
}

#[derive(Clone, Serialize, Deserialize)]
pub enum Rank {
    Lecturer,
    Professor,
    PRAG,
    ATER,
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
