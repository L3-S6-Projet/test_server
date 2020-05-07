use bimap::BiMap;
use log::{error, info};
use rand::{self, Rng};
use serde::{Deserialize, Serialize};
use std::io::Read;
use std::{
    collections::HashMap,
    fs::File,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use super::{
    models::Class, seed::seed_db, username_from_name, ClassUpdate, ClassroomUpdate, Database,
    NewClass, NewClassroom, NewOccupancySeed, NewSubject, SubjectUpdate, UpdateStatus, PAGE_SIZE,
};
use crate::{
    groups,
    models::{
        Classroom, Modification, ModificationOccupancy, ModificationType, Occupancy,
        StudentSubject, Subject, SubjectTeacher, User, UserKind,
    },
    NewOccupancy,
};

#[derive(Debug)]
struct BincodeError {
    kind: bincode::Error,
}

impl std::fmt::Display for BincodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.kind.fmt(f)
    }
}

impl std::error::Error for BincodeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        None
    }
}

#[derive(Serialize, Deserialize)]
pub struct JSONDatabase {
    dirty: bool,
    filename: String,
    delay: Duration,
    users: HashMap<String, User>,
    tokens: BiMap<String, String>,
    classrooms: HashMap<u32, Classroom>,
    classes: HashMap<u32, Class>,
    subjects: HashMap<u32, Subject>,
    subjects_teachers: HashMap<u32, SubjectTeacher>,
    subjects_students: HashMap<u32, StudentSubject>,
    occupancies: HashMap<u32, Occupancy>,
    modifications: HashMap<u32, Vec<Modification>>,
    next_user_id: u32,
    next_classroom_id: u32,
    next_class_id: u32,
    next_subject_id: u32,
    next_subject_teacher_id: u32,
    next_subject_students_id: u32,
    next_occupancy_id: u32,
}

impl JSONDatabase {
    pub fn new(filename: String) -> Self {
        // Try to read from disk
        match Self::from_file(&filename) {
            Ok(db) => {
                info!("Database loaded");
                return db;
            }
            Err(e) => {
                error!("Error while loading DB : {}", e);
                info!("Creating database..");
            }
        };

        let mut db = Self {
            dirty: true,
            filename,
            delay: Duration::from_millis(0),
            users: HashMap::new(),
            tokens: BiMap::new(),
            classrooms: HashMap::new(),
            classes: HashMap::new(),
            subjects: HashMap::new(),
            subjects_teachers: HashMap::new(),
            subjects_students: HashMap::new(),
            occupancies: HashMap::new(),
            modifications: HashMap::new(),
            next_user_id: 0,
            next_classroom_id: 0,
            next_class_id: 0,
            next_subject_id: 0,
            next_subject_teacher_id: 0,
            next_subject_students_id: 0,
            next_occupancy_id: 0,
        };

        db.reset();

        db
    }
    fn from_file(filename: &str) -> Result<Self, std::io::Error> {
        let contents = {
            let mut file = File::open(filename)?;
            let mut contents = Vec::new();
            file.read_to_end(&mut contents)?;
            contents
        };

        match bincode::deserialize(&contents[..]) {
            Ok(deserialized) => Ok(deserialized),
            Err(e) => Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                BincodeError { kind: e },
            )),
        }
    }

    pub fn set_dirty(&mut self) {
        self.dirty = true;
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn dirty_to_bincode(&mut self) -> Vec<u8> {
        /*let mut output = File::create(&self.filename)?;
        write!(output, "{}", self.dump_as_json()?)?;*/

        self.dirty = false;

        bincode::serialize(self).expect("could not serialize")
    }
}

impl Database for JSONDatabase {
    fn delay_set(&mut self, delay: Duration) {
        self.delay = delay;
        self.set_dirty();
    }

    fn delay_get(&self) -> Duration {
        self.delay
    }

    fn reset(&mut self) {
        self.dirty = true;
        self.delay = Duration::from_millis(0);
        self.users.clear();
        self.tokens.clear();
        self.classrooms.clear();
        self.classes.clear();
        self.subjects.clear();
        self.subjects_teachers.clear();
        self.subjects_students.clear();
        self.occupancies.clear();
        self.modifications.clear();
        self.next_user_id = 0;
        self.next_classroom_id = 0;
        self.next_class_id = 0;
        self.next_subject_id = 0;
        self.next_subject_teacher_id = 0;
        self.next_subject_students_id = 0;
        self.next_occupancy_id = 0;

        // Will call self.seed(), which calls persist
        seed_db(self);
    }

    fn seed(
        &mut self,
        users: impl Iterator<Item = super::NewUser>,
        classrooms: impl Iterator<Item = NewClassroom>,
        classes: impl Iterator<Item = NewClass>,
        subjects: impl Iterator<Item = NewSubject>,
        occupancies: impl Iterator<Item = NewOccupancySeed>,
    ) {
        classrooms.for_each(|c| self._classroom_add(c));
        users.for_each(|u| {
            self._user_add(u);
        });
        classes.for_each(|c| self._class_add(c));
        subjects.for_each(|s| self._subject_add(s));

        // Link students to each subjects
        let student_ids: Vec<u32> = self
            .users
            .values()
            .filter(|u| match u.kind {
                UserKind::Student(_) => true,
                UserKind::Administrator => false,
                UserKind::Teacher(_) => false,
            })
            .map(|s| s.id)
            .collect();

        let subject_ids: Vec<u32> = self.subjects.values().map(|s| s.id).collect();

        for student_id in student_ids {
            for subject_id in &subject_ids {
                self._subject_add_student(*subject_id, student_id);
            }
        }

        for new_occupancy in occupancies {
            let classroom_id = self
                .classrooms
                .iter()
                .find(|(_, c)| c.name == new_occupancy.classroom_name)
                .expect("classroom should exist")
                .0;

            let subject_id = self
                .subjects
                .iter()
                .find(|(_, s)| s.name == new_occupancy.subject_name)
                .expect("subject should exist")
                .0;

            let teacher_id = self
                .users
                .iter()
                .filter(|(_, u)| match u.kind {
                    UserKind::Student(_) => false,
                    UserKind::Administrator => false,
                    UserKind::Teacher(_) => true,
                })
                .find(|(_, u)| {
                    u.first_name == new_occupancy.teacher_first_name
                        && u.last_name == new_occupancy.teacher_last_name
                })
                .expect("teacher should exist")
                .1
                .id;

            let occupancy = NewOccupancy {
                classroom_id: Some(*classroom_id),
                group_number: new_occupancy.group_number,
                subject_id: Some(*subject_id),
                teacher_id,
                start_datetime: new_occupancy.start_datetime,
                end_datetime: new_occupancy.end_datetime,
                occupancy_type: new_occupancy.occupancy_type,
                name: new_occupancy.name,
            };

            self._add_occupancy(occupancy);
        }

        self.set_dirty();
    }

    fn dump_as_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(&self)
    }

    fn auth_login(&mut self, username: &str, password: &str) -> Option<(&User, String)> {
        let user_password = self.users.get(username).map(|u| u.password.to_string())?;

        if password != user_password {
            return None;
        }

        let mut rng = rand::thread_rng();

        let token: String = std::iter::repeat(())
            .map(|()| rng.sample(rand::distributions::Alphanumeric))
            .take(25)
            .collect();

        self.tokens.insert(token.clone(), username.to_string());
        self.set_dirty();

        let user = self
            .users
            .get(username)
            .expect("should be a valid reference");

        Some((user, token))
    }

    fn auth_logout(&mut self, token: &str) -> bool {
        let removed = self.tokens.remove_by_left(&token.to_string()).is_some();
        self.set_dirty();
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
            Some(page),
            query,
            |_| true,
        )
    }

    fn classroom_get(&self, id: u32) -> Option<&Classroom> {
        self.classrooms.get(&id)
    }

    fn classroom_add(&mut self, classroom: NewClassroom) {
        self._classroom_add(classroom);
        self.set_dirty();
    }

    fn classroom_remove(&mut self, classrooms: &[u32]) -> bool {
        // Check first
        if !classrooms.iter().all(|id| self.classrooms.contains_key(id)) {
            return false;
        }

        classrooms.iter().for_each(|id| {
            self.classrooms.remove(id);
        });

        self.set_dirty();

        true
    }

    fn classroom_update(&mut self, id: u32, update: ClassroomUpdate) -> UpdateStatus {
        let classroom = self.classrooms.get_mut(&id);

        if let Some(classroom) = classroom {
            let mut updated = false;

            if let Some(new_name) = update.name {
                classroom.name = new_name;
                updated = true;
                self.set_dirty();
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
        self.set_dirty();
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
        self.set_dirty();
    }

    fn user_list(
        &self,
        page: Option<usize>,
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
        self.set_dirty();
        true
    }

    fn class_add(&mut self, class: NewClass) {
        self._class_add(class);
        self.set_dirty();
    }

    fn class_list(&self, page: usize, query: Option<&str>) -> (usize, Vec<&Class>) {
        _search(
            self.classes.values(),
            |c: &Class| c.name.to_string(),
            Some(page),
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

        self.set_dirty();

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
                self.set_dirty();
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

    fn subject_list(
        &self,
        page: usize,
        query: Option<&str>,
        filter: impl Fn(&Subject) -> bool,
    ) -> (usize, Vec<&Subject>) {
        _search(
            self.subjects.values(),
            |s: &Subject| s.name.to_string(),
            Some(page),
            query,
            filter,
        )
    }

    fn subject_add(&mut self, subject: NewSubject) {
        self._subject_add(subject);
        self.set_dirty();
    }

    fn subject_remove(&mut self, subjects: &[u32]) -> bool {
        // TODO: what about subject_teacher?

        // Check first
        if !subjects.iter().all(|id| self.subjects.contains_key(id)) {
            return false;
        }

        subjects.iter().for_each(|id| {
            self.subjects.remove(id);
        });

        self.set_dirty();

        true
    }

    fn subject_get(&self, id: u32) -> Option<&Subject> {
        self.subjects.get(&id)
    }

    fn subject_students(&self, subject_id: u32) -> Vec<&User> {
        let ids: Vec<u32> = self
            .subjects_students
            .values()
            .filter(|subject_student| subject_student.subject_id == subject_id)
            .map(|subject_student| subject_student.student_id)
            .collect();

        let mut users = Vec::new();

        for id in ids {
            users.push(self.user_get_by_id(id).expect("user should exist"))
        }

        users
    }

    fn subject_add_student(&mut self, subject_id: u32, student_id: u32) {
        if self._subject_add_student(subject_id, student_id) {
            self.set_dirty();
        }
    }

    fn teacher_teaches(&self, teacher_id: u32, subject_id: u32) -> bool {
        self.subjects_teachers.values().any(|subject_teacher| {
            subject_teacher.teacher_id == teacher_id && subject_teacher.subject_id == subject_id
        })
    }

    fn teacher_in_charge(&self, teacher_id: u32, subject_id: u32) -> bool {
        self.subjects_teachers.values().any(|subject_teacher| {
            subject_teacher.teacher_id == teacher_id
                && subject_teacher.subject_id == subject_id
                && subject_teacher.in_charge
        })
    }

    fn teacher_subjects(&self, teacher_id: u32) -> Vec<&Subject> {
        let subject_ids: Vec<u32> = self
            .subjects_teachers
            .values()
            .filter(|st| st.teacher_id == teacher_id)
            .map(|st| st.subject_id)
            .collect();

        let mut subjects = Vec::new();

        for id in subject_ids {
            subjects.push(self.subject_get(id).expect("subject should exist"));
        }

        subjects
    }

    fn student_subjects(&self, student_id: u32) -> Vec<&Subject> {
        let subject_ids: Vec<u32> = self
            .subjects_students
            .values()
            .filter(|ss| ss.student_id == student_id)
            .map(|ss| ss.subject_id)
            .collect();

        let mut subjects = Vec::new();

        for id in subject_ids {
            subjects.push(self.subject_get(id).expect("subject should exist"));
        }

        subjects
    }

    fn subject_update(&mut self, id: u32, update: SubjectUpdate) -> UpdateStatus {
        let subject = self.subjects.get_mut(&id);

        if let Some(subject) = subject {
            let mut updated = false;

            if let Some(class_id) = update.class_id {
                subject.class_id = class_id;
                updated = true;
            }

            if let Some(name) = update.name {
                subject.name = name;
                updated = true;
            }

            if let Some(teacher_in_charge_id) = update.teacher_in_charge_id {
                // First, set in_charge to false for the existing teacher in charge.
                let old_teacher_in_charge_id = self
                    .subjects_teachers
                    .values()
                    .filter(|st| st.subject_id == id)
                    .find(|st| self.teacher_in_charge(st.teacher_id, id))
                    .map(|st| st.teacher_id)
                    .expect("a subject should always have a teacher in charge");

                self._set_teaches(id, old_teacher_in_charge_id, Some(false));

                // Then, set the new teacher in charge
                self._set_teaches(id, teacher_in_charge_id, Some(true));

                updated = true;
            }

            if updated {
                self.set_dirty();
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

    fn subject_add_group(&mut self, subject_id: u32) {
        let mut subject = self
            .subjects
            .get_mut(&subject_id)
            .expect("subject shoulld exist");
        subject.group_count += 1;
        self.set_dirty();
    }

    fn subject_remove_group(&mut self, subject_id: u32) -> bool {
        let mut subject = self
            .subjects
            .get_mut(&subject_id)
            .expect("subject should exist");

        if subject.group_count >= 2 {
            subject.group_count -= 1;
            self.set_dirty();
            true
        } else {
            false
        }
    }

    /// Adds a teacher to a subject
    fn teacher_set_teaches(&mut self, teacher_id: u32, subject_id: u32) {
        self._set_teaches(subject_id, teacher_id, None);
        self.set_dirty();
    }

    fn teacher_unset_teaches(&mut self, teacher_id: u32, subject_id: u32) {
        self._unset_teaches(subject_id, teacher_id);
        self.set_dirty();
    }

    fn distribute_subject_groups(&mut self, subject_id: u32) {
        self._distribute_subject_groups(subject_id);
        self.set_dirty();
    }

    fn student_group(&self, student_id: u32, subject_id: u32) -> u32 {
        self.subjects_students
            .values()
            .find(|ss| ss.student_id == student_id && ss.subject_id == subject_id)
            .expect("student subject should exist")
            .group_number
    }

    fn occupancies_list(&self, from: Option<u64>, to: Option<u64>) -> Vec<&Occupancy> {
        self.occupancies
            .values()
            .filter(|o| {
                if let Some(from) = from {
                    if o.start_datetime < from {
                        return false;
                    }
                }

                if let Some(to) = to {
                    if o.end_datetime > to {
                        return false;
                    }
                }

                true
            })
            .collect()
    }

    fn occupancies_remove(&mut self, occupancies: &[u32]) -> bool {
        // Check first
        if !occupancies
            .iter()
            .all(|id| self.occupancies.contains_key(id))
        {
            return false;
        }

        occupancies.iter().for_each(|id| {
            self.occupancies.remove(id);
        });

        self.set_dirty();

        true
    }

    fn occupancies_add(&mut self, occupancy: NewOccupancy) {
        self._add_occupancy(occupancy);
        self.set_dirty();
    }

    fn classroom_free(&self, classroom_id: u32, from: u64, to: u64) -> bool {
        // Find an occupancy that is in this classroom, and between from and to
        !self.occupancies.values().any(|o| {
            o.classroom_id == Some(classroom_id) && o.start_datetime >= from && o.end_datetime <= to
        })
    }

    fn teacher_free(&self, teacher_id: u32, from: u64, to: u64) -> bool {
        !self
            .occupancies
            .values()
            .any(|o| o.teacher_id == teacher_id && o.start_datetime >= from && o.end_datetime <= to)
    }

    fn class_free(&self, class_id: u32, from: u64, to: u64) -> bool {
        !self.occupancies.values().any(|o| {
            // Find an occupancy with a subject
            let subject = match o.subject_id.and_then(|sid| self.subject_get(sid)) {
                Some(subject) => subject,
                None => return false,
            };

            subject.class_id == class_id && o.start_datetime >= from && o.end_datetime <= to
        })
    }

    fn group_free(
        &self,
        class_id: u32,
        subject_id: u32,
        group_number: u32,
        from: u64,
        to: u64,
    ) -> bool {
        !self.occupancies.values().any(|o| {
            // Find an occupancy with a subject
            let subject = match o.subject_id.and_then(|sid| self.subject_get(sid)) {
                Some(subject) => subject,
                None => return false,
            };

            subject.id == subject_id
                && subject.class_id == class_id
                && o.start_datetime >= from
                && o.end_datetime <= to
                && o.group_number == Some(group_number)
        })
    }

    fn last_occupancies_modifications(&self, user_id: u32) -> Vec<&Modification> {
        self.modifications
            .get(&user_id)
            .map(|v| v.iter().collect())
            .unwrap_or(Vec::new())
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

    fn _subject_add(&mut self, new_subject: NewSubject) {
        let subject = Subject {
            id: self.next_subject_id,
            name: new_subject.name,
            class_id: new_subject.class_id,
            group_count: 1,
        };

        let teacher_id = new_subject.teacher_in_charge_id;

        let subject_teacher = SubjectTeacher {
            id: self.next_subject_teacher_id,
            teacher_id,
            subject_id: subject.id,
            in_charge: true,
        };

        self.subjects.insert(self.next_subject_id, subject);
        self.subjects_teachers
            .insert(self.next_subject_teacher_id, subject_teacher);
        self.next_subject_id += 1;
        self.next_subject_teacher_id += 1;
    }

    /// If in_charge is set, it will overwrite ; else, it will not overwrite and if the row needs
    /// to be created, it will be set to false.
    fn _set_teaches(&mut self, subject_id: u32, teacher_id: u32, in_charge: Option<bool>) {
        let subject_teacher = self
            .subjects_teachers
            .values_mut()
            .find(|v| v.subject_id == subject_id && v.teacher_id == teacher_id);

        // Already teaches : only set in_charge
        if let Some(subject_teacher) = subject_teacher {
            if let Some(in_charge) = in_charge {
                subject_teacher.in_charge = in_charge;
            }

            return;
        }

        // Else: create the relation

        let subject_teacher = SubjectTeacher {
            id: self.next_subject_teacher_id,
            teacher_id,
            subject_id,
            in_charge: in_charge.unwrap_or(false),
        };

        self.subjects_teachers
            .insert(self.next_subject_teacher_id, subject_teacher);

        self.next_subject_teacher_id += 1;
    }

    fn _set_in_charge(&mut self, subject_id: u32, teacher_id: u32, in_charge: bool) -> bool {
        let subject_teacher = self
            .subjects_teachers
            .values_mut()
            .find(|v| v.subject_id == subject_id && v.teacher_id == teacher_id);

        match subject_teacher {
            Some(subject_teacher) => {
                subject_teacher.in_charge = in_charge;
                true
            }
            None => false,
        }
    }

    fn _unset_teaches(&mut self, subject_id: u32, teacher_id: u32) -> bool {
        let id = self
            .subjects_teachers
            .iter()
            .find(|(_, v)| v.subject_id == subject_id && v.teacher_id == teacher_id)
            .map(|(k, _)| *k);

        if let Some(id) = id {
            self.subjects_teachers.remove(&id);
            true
        } else {
            false
        }
    }

    fn _distribute_subject_groups(&mut self, subject_id: u32) {
        let group_count = self
            .subject_get(subject_id)
            .expect("subject should exist.")
            .group_count;

        // All students, sorted by name
        let mut students: Vec<&User> = self.subject_students(subject_id).clone();
        students.sort_by_key(|s| s.full_name());
        let student_ids: Vec<u32> = students.iter().map(|s| s.id).collect();

        // Assign groups
        let groups = groups(student_ids.len(), group_count);

        for (student_id, group_number) in student_ids.iter().zip(groups.iter()) {
            let mut student_subject = self
                .subjects_students
                .values_mut()
                .find(|subject_student| {
                    subject_student.student_id == *student_id
                        && subject_student.subject_id == subject_id
                })
                .expect("student should participate in the subject (checked earlier)");

            student_subject.group_number = *group_number;
        }
    }

    fn _subject_add_student(&mut self, subject_id: u32, student_id: u32) -> bool {
        let exists = self
            .subjects_students
            .values()
            .find(|subject_student| {
                subject_student.subject_id == subject_id && subject_student.student_id == student_id
            })
            .is_some();

        if !exists {
            let subject_student = StudentSubject {
                id: self.next_subject_students_id,
                student_id,
                subject_id,
                group_number: 0, // TODO!!!
            };

            self.subjects_students
                .insert(self.next_subject_students_id, subject_student);
            self.next_subject_students_id += 1;

            true
        } else {
            false
        }
    }

    fn _add_occupancy(&mut self, occupancy: NewOccupancy) {
        let occupancy = Occupancy {
            id: self.next_occupancy_id,
            classroom_id: occupancy.classroom_id,
            group_number: occupancy.group_number,
            subject_id: occupancy.subject_id,
            teacher_id: occupancy.teacher_id,
            start_datetime: occupancy.start_datetime,
            end_datetime: occupancy.end_datetime,
            occupancy_type: occupancy.occupancy_type,
            name: occupancy.name,
        };

        // Initialize with teacher id
        let mut affected_users: Vec<u32> = vec![occupancy.teacher_id];

        // Add subject id
        if let Some(subject_id) = occupancy.subject_id {
            // Find each student in the subject
            let ss: Vec<(u32, u32)> = self
                .subjects_students
                .values()
                .filter(|ss| ss.subject_id == subject_id)
                .map(|ss| {
                    let student = self
                        .user_get_student_by_id(ss.student_id)
                        .expect("should be a valid reference");

                    (ss.group_number, student.id)
                })
                .collect();

            // If there is a group, then only choose those in that group
            if let Some(group_number) = occupancy.group_number {
                affected_users.extend(
                    ss.iter()
                        .filter(|(user_group_number, _)| user_group_number == &group_number)
                        .map(|(_, uid)| uid),
                );
            } else {
                affected_users.extend(ss.iter().map(|(_, uid)| uid));
            }
        }

        let start = SystemTime::now();
        let since_the_epoch = start
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards");

        let subject = occupancy
            .subject_id
            .map(|id| self.subject_get(id).expect("should be a valid reference"));

        let modification = Modification {
            modification_type: ModificationType::Create,
            modification_timestamp: since_the_epoch.as_secs(),
            occupancy: ModificationOccupancy {
                subject_id: occupancy.subject_id,
                class_id: subject.map(|s| s.class_id),
                occupancy_type: occupancy.occupancy_type.clone(),
                occupancy_start: occupancy.start_datetime,
                occupancy_end: occupancy.end_datetime,
                previous_occupancy_start: occupancy.start_datetime,
                previous_occupancy_end: occupancy.end_datetime,
            },
        };

        self._add_modification(&affected_users[..], modification);

        self.occupancies.insert(self.next_occupancy_id, occupancy);
        self.next_occupancy_id += 1;
    }

    fn _add_modification(&mut self, affected_users: &[u32], modification: Modification) {
        // TODO: keep to only last 25
        for uid in affected_users {
            let vec = self.modifications.entry(*uid).or_insert(Vec::new());
            vec.insert(0, modification.clone());
            vec.truncate(25);
        }
    }
}

fn _search<'a, T, F>(
    collection: impl Iterator<Item = &'a T>,
    property: F,
    page: Option<usize>,
    query: Option<&str>,
    custom_filter: impl Fn(&T) -> bool,
) -> (usize, Vec<&'a T>)
where
    F: Fn(&T) -> String,
{
    let mut filter = contains_query(query, property);

    // If no page arg is provided, then return the whole collection.
    let page = match page {
        Some(page) => page,
        None => {
            let vec: Vec<&T> = collection
                .filter(|row| filter(&row) && custom_filter(&row))
                .collect();

            return (vec.len(), vec);
        }
    };

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
