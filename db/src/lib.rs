use serde::Deserialize;
use std::{sync::Arc, time::Duration};
use tokio::sync::{Mutex, MutexGuard};

mod assets;
mod json;
pub mod models;
mod seed;
mod utils;

use json::JSONDatabase;
use models::{
    Class, ClassLevel, Classroom, Modification, Occupancy, OccupancyType, Subject, User, UserKind,
};

pub const PAGE_SIZE: usize = 10;

pub type ConcreteDb = JSONDatabase;
pub type Db = Arc<Mutex<ConcreteDb>>;
pub type LockedDb<'a> = MutexGuard<'a, ConcreteDb>;

pub fn new_db(filename: String) -> Db {
    Arc::new(Mutex::new(JSONDatabase::new(filename)))
}

// While the trait is not used at runtime, it allows checking that the impls are complete
pub trait Database {
    fn from_file(filename: &str) -> Result<ConcreteDb, std::io::Error>;
    fn reset(&mut self);
    fn seed(
        &mut self,
        users: impl Iterator<Item = NewUser>,
        classrooms: impl Iterator<Item = NewClassroom>,
        classes: impl Iterator<Item = NewClass>,
        subjects: impl Iterator<Item = NewSubject>,
        occupancies: impl Iterator<Item = NewOccupancySeed>,
    );
    fn dump_as_json(&self) -> Result<String, serde_json::Error>;

    fn delay_set(&mut self, delay: Duration);
    fn delay_get(&self) -> Duration;

    fn auth_login(&mut self, username: &str, password: &str) -> Option<(&User, String)>;
    fn auth_logout(&mut self, token: &str) -> bool;
    fn auth_get_user<'a, 'b>(&'a self, token: &str) -> Option<&'a User>;

    fn classroom_list(&self, page: usize, query: Option<&str>) -> (usize, Vec<&Classroom>);
    fn classroom_get(&self, id: u32) -> Option<&Classroom>;
    fn classroom_add(&mut self, classroom: NewClassroom);
    fn classroom_remove(&mut self, classrooms: &[u32]) -> bool;
    fn classroom_update(&mut self, id: u32, update: ClassroomUpdate) -> UpdateStatus;

    fn user_add(&mut self, user: NewUser) -> &User;
    fn user_get(&self, username: &str) -> Option<&User>;
    fn user_get_by_id(&self, id: u32) -> Option<&User>;
    fn user_update(&mut self, user: User);
    fn user_list(
        &self,
        page: Option<usize>,
        query: Option<&str>,
        filter: impl Fn(&User) -> bool,
    ) -> (usize, Vec<&User>);
    fn user_remove(&mut self, users: &[u32]) -> bool;

    fn user_get_teacher_by_id(&self, id: u32) -> Option<&User> {
        let user = self.user_get_by_id(id)?;

        let is_teacher = match &user.kind {
            UserKind::Administrator => false,
            UserKind::Teacher(_) => true,
            UserKind::Student(_) => false,
        };

        if is_teacher {
            Some(user)
        } else {
            None
        }
    }

    fn user_get_student_by_id(&self, id: u32) -> Option<&User> {
        let user = self.user_get_by_id(id)?;

        let is_student = match &user.kind {
            UserKind::Administrator => false,
            UserKind::Teacher(_) => false,
            UserKind::Student(_) => true,
        };

        if is_student {
            Some(user)
        } else {
            None
        }
    }

    fn class_list(&self, page: usize, query: Option<&str>) -> (usize, Vec<&Class>);
    fn class_add(&mut self, class: NewClass);
    fn class_remove(&mut self, classes: &[u32]) -> bool;
    fn class_get(&self, id: u32) -> Option<&Class>;
    fn class_update(&mut self, id: u32, update: ClassUpdate) -> UpdateStatus;

    fn subject_list(
        &self,
        page: usize,
        query: Option<&str>,
        filter: impl Fn(&Subject) -> bool,
    ) -> (usize, Vec<&Subject>);
    fn subject_add(&mut self, subject: NewSubject);
    fn subject_remove(&mut self, subjects: &[u32]) -> bool;
    fn subject_get(&self, id: u32) -> Option<&Subject>;
    fn subject_update(&mut self, id: u32, update: SubjectUpdate) -> UpdateStatus;
    fn subject_students(&self, subject_id: u32) -> Vec<&User>;
    fn subject_add_student(&mut self, subject_id: u32, student_id: u32);
    fn subject_add_group(&mut self, subject_id: u32);
    fn subject_remove_group(&mut self, subject_id: u32) -> bool;

    fn teacher_teaches(&self, teacher_id: u32, subject_id: u32) -> bool;
    fn teacher_in_charge(&self, teacher_id: u32, subject_id: u32) -> bool;
    fn teacher_set_teaches(&mut self, teacher_id: u32, subject_id: u32);
    fn teacher_unset_teaches(&mut self, teacher_id: u32, subject_id: u32);
    fn teacher_subjects(&self, teacher_id: u32) -> Vec<&Subject>;
    fn student_subjects(&self, student_id: u32) -> Vec<&Subject>;
    fn student_subjects_with_groups(&self, student_id: u32) -> Vec<(&Subject, u32)>;
    fn student_group(&self, student_id: u32, subject_id: u32) -> u32;

    fn distribute_subject_groups(&mut self, subject_id: u32);

    fn occupancies_list(&self, from: Option<u64>, to: Option<u64>) -> Vec<&Occupancy>;
    fn occupancies_remove(&mut self, occupancies: &[u32]) -> bool;
    fn occupancies_add(&mut self, occupancy: NewOccupancy);
    fn occupancies_update(&mut self, id: u32, update: OccupancyUpdate) -> UpdateStatus;

    fn classroom_free(&self, classroom_id: u32, from: u64, to: u64) -> bool;
    fn teacher_free(&self, teacher_id: u32, from: u64, to: u64) -> bool;
    fn class_free(&self, class_id: u32, from: u64, to: u64) -> bool;
    fn group_free(
        &self,
        class_id: u32,
        subject_id: u32,
        group_number: u32,
        from: u64,
        to: u64,
    ) -> bool;

    fn last_occupancies_modifications(&self, user_id: u32) -> Vec<&Modification>;
}

pub fn username_from_name(first_name: &str, last_name: &str) -> String {
    unidecode::unidecode(&format!("{} {}", last_name, first_name))
        .to_ascii_lowercase()
        .replace(" ", ".")
}

pub struct NewUser {
    pub first_name: String,
    pub last_name: String,
    pub password: String,
    pub kind: UserKind,
}

#[derive(Deserialize)]
pub struct NewClassroom {
    pub name: String,
    pub capacity: u16,
}

#[derive(Deserialize)]
pub struct ClassroomUpdate {
    pub name: Option<String>,
}

pub struct UpdateStatus {
    pub found: bool,
    pub updated: bool,
}

#[derive(Deserialize)]
pub struct NewClass {
    pub name: String,
    pub level: ClassLevel,
}

#[derive(Deserialize)]
pub struct ClassUpdate {
    pub name: Option<String>,
    pub level: Option<ClassLevel>,
}

#[derive(Deserialize)]
pub struct NewSubject {
    pub class_id: u32,
    pub name: String,
    pub teacher_in_charge_id: u32,
}

#[derive(Deserialize, Debug)]
pub struct SubjectUpdate {
    pub class_id: Option<u32>, // TODO: remove
    pub name: Option<String>,
    pub teacher_in_charge_id: Option<u32>,
}

/// [0, 0, 0, 1, 1, 1, 2, 2, 2]
pub fn groups(student_count: usize, group_count: u32) -> Vec<u32> {
    let mut numbers = Vec::new();

    for (index, group) in group_numbers(student_count, group_count).iter().enumerate() {
        for _ in 0..*group {
            numbers.push(index as u32);
        }
    }

    numbers
}

pub fn group_numbers(student_count: usize, group_count: u32) -> Vec<u32> {
    let mut remaining = student_count;
    let mut index = 0;
    let mut groups = vec![0; group_count as usize];

    while remaining > 0 {
        groups[index] += 1;
        index += 1;
        if index == group_count as usize {
            index = 0;
        }
        remaining -= 1;
    }

    groups
}

/// [3, 3, 3]
pub fn old_group_numbers(student_count: usize, group_count: u32) -> Vec<u32> {
    let mut groups = Vec::with_capacity(group_count as usize);
    let mut sum = 0;

    for i in 0..group_count {
        if i == group_count - 1 {
            groups.push(student_count as u32 - sum);
        } else {
            sum += student_count as u32 / group_count;
            groups.push(student_count as u32 / group_count);
        }
    }

    groups
}

pub struct NewOccupancy {
    pub classroom_id: Option<u32>,
    pub group_number: Option<u32>,
    pub subject_id: Option<u32>,
    pub teacher_id: u32,
    pub start_datetime: u64,
    pub end_datetime: u64,
    pub occupancy_type: OccupancyType,
    pub name: String,
}

pub struct NewOccupancySeed {
    pub classroom_name: String,
    pub group_number: Option<u32>,
    pub subject_name: String,
    pub teacher_first_name: String,
    pub teacher_last_name: String,
    pub start_datetime: u64,
    pub end_datetime: u64,
    pub occupancy_type: OccupancyType,
    pub name: String,
}

#[derive(Deserialize)]
pub struct OccupancyUpdate {
    pub classroom_id: Option<u32>,
    pub start: Option<u64>,
    pub end: Option<u64>,
    pub name: Option<String>,
}
