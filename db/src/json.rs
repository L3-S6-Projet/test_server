use bimap::BiMap;
use rand::{self, Rng};
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::{collections::HashMap, fs::File, time::Duration};

use super::{
    models::Class, seed::seed_db, username_from_name, ClassUpdate, ClassroomUpdate, Database,
    NewClass, NewClassroom, UpdateStatus, PAGE_SIZE,
};
use crate::models::{Classroom, User};

#[derive(Serialize, Deserialize)]
pub struct JSONDatabase {
    filename: String,
    delay: Duration,
    users: HashMap<String, User>,
    tokens: BiMap<String, String>,
    classrooms: HashMap<u32, Classroom>,
    classes: HashMap<u32, Class>,
    next_user_id: u32,
    next_classroom_id: u32,
    next_class_id: u32,
}

impl JSONDatabase {
    pub fn new(filename: String) -> Self {
        // Try to read from disk
        if let Ok(db) = Self::from_file(&filename) {
            return db;
        }

        let mut db = Self {
            filename,
            delay: Duration::from_millis(0),
            users: HashMap::new(),
            tokens: BiMap::new(),
            classrooms: HashMap::new(),
            classes: HashMap::new(),
            next_user_id: 0,
            next_classroom_id: 0,
            next_class_id: 0,
        };

        db.reset();

        db
    }
    fn from_file(filename: &str) -> Result<Self, std::io::Error> {
        let contents = {
            let mut file = File::open(filename)?;
            let mut contents = String::new();
            file.read_to_string(&mut contents)?;
            contents
        };

        Ok(serde_json::from_str(&contents)?)
    }

    fn persist(&self) -> Result<(), std::io::Error> {
        let mut output = File::create(&self.filename)?;
        write!(output, "{}", self.dump_as_json()?)?;
        Ok(())
    }
}

impl Database for JSONDatabase {
    fn delay_set(&mut self, delay: Duration) {
        self.delay = delay;
        self.persist().expect("could not save DB")
    }

    fn delay_get(&self) -> Duration {
        self.delay
    }

    fn reset(&mut self) {
        self.delay = Duration::from_millis(0);
        self.users.clear();
        self.tokens.clear();
        self.classrooms.clear();
        self.classes.clear();
        self.next_user_id = 0;
        self.next_classroom_id = 0;
        self.next_class_id = 0;

        seed_db(self);

        self.persist().expect("could not save DB");
    }

    fn seed(
        &mut self,
        users: impl Iterator<Item = super::NewUser>,
        classrooms: impl Iterator<Item = NewClassroom>,
        classes: impl Iterator<Item = NewClass>,
    ) {
        classrooms.for_each(|c| self._classroom_add(c));
        users.for_each(|u| {
            self._user_add(u);
        });
        classes.for_each(|c| self._class_add(c));
        self.persist().expect("could not save DB");
    }

    fn dump_as_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(&self)
    }

    fn auth_login(&mut self, username: &str, password: &str) -> Option<(&User, String)> {
        let user = self.users.get(username)?;

        if password == user.password {
            let mut rng = rand::thread_rng();
            let token: String = std::iter::repeat(())
                .map(|()| rng.sample(rand::distributions::Alphanumeric))
                .take(25)
                .collect();

            self.tokens.insert(token.clone(), user.username.clone());
            self.persist().expect("could not save DB");
            Some((user, token))
        } else {
            None
        }
    }

    fn auth_logout(&mut self, token: &str) -> bool {
        let removed = self.tokens.remove_by_left(&token.to_string()).is_some();
        self.persist().expect("could not save DB");
        removed
    }

    fn auth_get_user<'a, 'b>(&'a self, token: &str) -> Option<&'a User> {
        let username = self.tokens.get_by_left(&token.to_string())?; // TODO
        self.users.get(username)
    }

    fn classroom_list(&self, page: usize, query: Option<&str>) -> (usize, Vec<&Classroom>) {
        _search(
            self.classrooms.values(),
            |c: &Classroom| c.name.to_string(),
            page,
            query,
            |_| true,
        )
    }

    fn classroom_get(&self, id: u32) -> Option<&Classroom> {
        self.classrooms.get(&id)
    }

    fn classroom_add(&mut self, classroom: NewClassroom) {
        self._classroom_add(classroom);
        self.persist().expect("could not save DB");
    }

    fn classroom_remove(&mut self, classrooms: &[u32]) -> bool {
        // Check first
        if !classrooms.iter().all(|id| self.classrooms.contains_key(id)) {
            return false;
        }

        classrooms.iter().for_each(|id| {
            self.classrooms.remove(id);
        });

        true
    }

    fn classroom_update(&mut self, id: u32, update: ClassroomUpdate) -> UpdateStatus {
        let classroom = self.classrooms.get_mut(&id);

        if let Some(classroom) = classroom {
            let mut updated = false;

            if let Some(new_name) = update.name {
                classroom.name = new_name;
                updated = true;
                self.persist().expect("could not save DB");
            }

            UpdateStatus {
                found: true,
                updated,
            }
        } else {
            UpdateStatus {
                found: false,
                updated: false,
            }
        }
    }

    fn user_add(&mut self, user: super::NewUser) -> &User {
        let username = self._user_add(user);
        self.persist().expect("could not save DB");
        self.users.get(&username).expect("user was just added")
    }

    fn user_get(&self, username: &str) -> Option<&User> {
        self.users.get(username)
    }

    fn user_get_by_id(&self, id: u32) -> Option<&User> {
        self.users.values().find(|u| u.id == id)
    }

    fn user_update(&mut self, user: User) {
        self.users.insert(user.username.clone(), user);
        self.persist().expect("could not save DB");
    }

    fn user_list(
        &self,
        page: usize,
        query: Option<&str>,
        filter: impl Fn(&User) -> bool,
    ) -> (usize, Vec<&User>) {
        _search(
            self.users.values(),
            |u: &User| u.full_name(),
            page,
            query,
            filter,
        )
    }

    fn user_remove(&mut self, users: &[u32]) -> bool {
        let all_users_ids: Vec<u32> = self.users.values().map(|u| u.id).collect();

        // Check first that all IDS exist
        if !users.iter().all(|id| all_users_ids.contains(id)) {
            return false;
        }

        let removed_usernames: Vec<String> = self
            .users
            .values()
            .filter(|u| users.contains(&u.id))
            .map(|u| u.username.clone())
            .collect();

        for username in removed_usernames {
            self.tokens.remove_by_right(&username);
        }

        self.users.retain(|_, u| !users.contains(&u.id));
        // TODO: persist
        self.persist().expect("could not save DB");
        true
    }

    fn class_add(&mut self, class: NewClass) {
        self._class_add(class);
        self.persist().expect("could not save DB");
    }

    fn class_list(&self, page: usize, query: Option<&str>) -> (usize, Vec<&Class>) {
        _search(
            self.classes.values(),
            |c: &Class| c.name.to_string(),
            page,
            query,
            |_| true,
        )
    }

    fn class_remove(&mut self, classes: &[u32]) -> bool {
        // Check first
        if !classes.iter().all(|id| self.classes.contains_key(id)) {
            return false;
        }

        classes.iter().for_each(|id| {
            self.classes.remove(id);
        });

        true
    }

    fn class_get(&self, id: u32) -> Option<&Class> {
        self.classes.get(&id)
    }

    fn class_update(&mut self, id: u32, update: ClassUpdate) -> UpdateStatus {
        let class = self.classes.get_mut(&id);

        if let Some(class) = class {
            let mut updated = false;

            if let Some(new_name) = update.name {
                class.name = new_name;
                updated = true;
            }

            if let Some(new_level) = update.level {
                class.level = new_level;
                updated = true;
            }

            if updated {
                self.persist().expect("could not save DB");
            }

            UpdateStatus {
                found: true,
                updated,
            }
        } else {
            UpdateStatus {
                found: false,
                updated: false,
            }
        }
    }
}

impl JSONDatabase {
    fn _user_add(&mut self, user: super::NewUser) -> String {
        let username = username_from_name(&user.first_name, &user.last_name);

        self.users.insert(
            username.clone(),
            User {
                id: self.next_user_id,
                first_name: user.first_name,
                last_name: user.last_name,
                username: username.clone(),
                password: user.password,
                kind: user.kind,
            },
        );

        self.next_user_id += 1;
        username
    }

    fn _classroom_add(&mut self, classroom: NewClassroom) {
        let classroom = Classroom {
            id: self.next_classroom_id,
            name: classroom.name,
            capacity: classroom.capacity,
        };

        self.classrooms.insert(self.next_classroom_id, classroom);
        self.next_classroom_id += 1;
    }

    fn _class_add(&mut self, class: NewClass) {
        let class = Class {
            id: self.next_class_id,
            name: class.name,
            level: class.level,
        };

        self.classes.insert(self.next_class_id, class);
        self.next_class_id += 1;
    }
}

fn _search<'a, T, F>(
    collection: impl Iterator<Item = &'a T>,
    property: F,
    page: usize,
    query: Option<&str>,
    custom_filter: impl Fn(&T) -> bool,
) -> (usize, Vec<&'a T>)
where
    F: Fn(&T) -> String,
{
    let mut filter = contains_query(query, property);
    let mut total = 0;
    let mut skipped = 0;
    let mut results: Vec<&T> = Vec::new();
    let to_skip = (page - 1) * PAGE_SIZE;

    for row in collection {
        if !filter(&row) || !custom_filter(&row) {
            continue;
        }

        total += 1;

        if skipped < to_skip {
            skipped += 1;
        } else if results.len() < PAGE_SIZE {
            results.push(row);
        }
    }

    (total, results)
}

/// Returns a function to be used as a filter that checks if the provided query is contained in the
/// object string.
fn contains_query<T, F>(query: Option<&str>, property: F) -> impl FnMut(&&T) -> bool
where
    F: Fn(&T) -> String,
{
    let normalize = |s: &str| unidecode::unidecode(s.trim()).to_ascii_lowercase();
    let query = query.map(|d| truncate(d, 50)).map(normalize);

    move |object: &&T| {
        if let Some(query) = &query {
            let name = property(object);
            let name = normalize(&name);
            name.contains(query)
        } else {
            true
        }
    }
}

fn truncate(s: &str, max_chars: usize) -> &str {
    match s.char_indices().nth(max_chars) {
        None => s,
        Some((idx, _)) => &s[..idx],
    }
}
