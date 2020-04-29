use std::{sync::Arc, time::Duration};
use tokio::sync::Mutex;

mod json;
pub mod models;
mod seed;

use json::JSONDatabase;
use models::{Classroom, User, UserKind};

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
        classrooms: impl Iterator<Item = Classroom>,
    );
    fn dump_as_json(&self) -> Result<String, serde_json::Error>;

    fn delay_set(&mut self, delay: Duration);
    fn delay_get(&self) -> Duration;

    fn auth_login(&mut self, username: &str, password: &str) -> Option<(&User, String)>;
    fn auth_logout(&mut self, token: &str) -> bool;
    fn auth_get_user<'a, 'b>(&'a self, token: &str) -> Option<&'a User>;

    fn classroom_list(&self) -> Vec<&Classroom>;
    fn classroom_get(&self, id: u32) -> Option<&Classroom>;
    fn classroom_add(&mut self, classroom: Classroom);

    fn users_add(&mut self, user: NewUser);
    fn users_get(&self, username: &str) -> Option<&User>;
    fn users_update(&mut self, user: User);
}

pub struct NewUser {
    pub first_name: String,
    pub last_name: String,
    pub username: String,
    pub password: String,
    pub kind: UserKind,
}
