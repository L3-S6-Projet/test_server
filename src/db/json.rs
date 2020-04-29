use bimap::BiMap;
use rand::{self, Rng};
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::{collections::HashMap, fs::File, time::Duration};

use super::{seed::seed_db, Database};
use crate::db::models::{Classroom, User};

#[derive(Serialize, Deserialize)]
pub struct JSONDatabase {
    filename: String,
    delay: Duration,
    users: HashMap<String, User>,
    tokens: BiMap<String, String>,
    classrooms: HashMap<u32, Classroom>,
    next_user_id: u32,
    next_classroom_id: u32,
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
            next_user_id: 0,
            next_classroom_id: 0,
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
        self.users = HashMap::new();
        self.tokens = BiMap::new();
        self.classrooms = HashMap::new();
        self.next_user_id = 0;
        self.next_classroom_id = 0;

        seed_db(self);

        self.persist().expect("could not save DB");
    }

    fn seed(
        &mut self,
        users: impl Iterator<Item = super::NewUser>,
        classrooms: impl Iterator<Item = Classroom>,
    ) {
        classrooms.for_each(|c| self._classroom_add(c));
        users.for_each(|u| self._users_add(u));
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

    fn classroom_list(&self) -> Vec<&Classroom> {
        self.classrooms.values().collect()
    }

    fn classroom_get(&self, id: u32) -> Option<&Classroom> {
        self.classrooms.get(&id)
    }

    fn classroom_add(&mut self, classroom: Classroom) {
        self._classroom_add(classroom);
        self.persist().expect("could not save DB");
    }

    fn users_add(&mut self, user: super::NewUser) {
        self._users_add(user);
        self.persist().expect("could not save DB")
    }

    fn users_get(&self, username: &str) -> Option<&User> {
        self.users.get(username)
    }

    fn users_update(&mut self, user: User) {
        self.users.insert(user.username.clone(), user);
        self.persist().expect("could not save DB");
    }
}

impl JSONDatabase {
    fn _users_add(&mut self, user: super::NewUser) {
        self.users.insert(
            user.username.clone(),
            User {
                id: self.next_user_id,
                first_name: user.first_name,
                last_name: user.last_name,
                username: user.username,
                password: user.password,
                kind: user.kind,
            },
        );

        self.next_user_id += 1;
    }

    fn _classroom_add(&mut self, classroom: Classroom) {
        self.classrooms.insert(self.next_classroom_id, classroom);
        self.next_classroom_id += 1;
    }
}
