use serde::Deserialize;
use std::{sync::Arc, time::Duration};
use tokio::sync::Mutex;

mod assets;
mod json;
pub mod models;
mod seed;
mod utils;

use json::JSONDatabase;
use models::{Class, ClassLevel, Classroom, User, UserKind};

pub const PAGE_SIZE: usize = 10;

pub type Db = Arc<Mutex<JSONDatabase>>;

pub fn new_db(filename: String) -> Db {
    Arc::new(Mutex::new(JSONDatabase::new(filename)))
}

// While the trait is not used at runtime, it allows checking that the impls are complete
pub trait Database {
    fn reset(&mut self);
    fn seed(
        &mut self,
        users: impl Iterator<Item = NewUser>,
        classrooms: impl Iterator<Item = NewClassroom>,
        classes: impl Iterator<Item = NewClass>,
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
        page: usize,
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

    fn class_list(&self, page: usize, query: Option<&str>) -> (usize, Vec<&Class>);
    fn class_add(&mut self, class: NewClass);
    fn class_remove(&mut self, classes: &[u32]) -> bool;
    fn class_get(&self, id: u32) -> Option<&Class>;
    fn class_update(&mut self, id: u32, update: ClassUpdate) -> UpdateStatus;
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
    pub capacity: u16,
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
