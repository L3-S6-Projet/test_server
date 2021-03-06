use super::{
    models::{Rank, StudentInformations, TeacherInformations, UserKind},
    username_from_name, ClassLevel, Database, NewClass, NewClassroom, NewSubject, NewUser,
};
use crate::assets::{Event, EventType, StudentName};
use crate::{models::OccupancyType, utils::UniqueExt, NewOccupancySeed};
use rand::{self, Rng};

pub fn seed_db<D: Database>(db: &mut D) {
    let events = Event::from_parsed_ical();
    let student_names = StudentName::from_parsed_json();

    let users = test_users();
    let classrooms = test_classrooms(&events);
    let teachers = test_teachers(&events);
    let classes = test_classes();
    let students = test_students(&student_names);
    let subjects = test_subjects(&events);

    let mut occupancies: Vec<NewOccupancySeed> = Vec::new();

    for event in events {
        let professor = match event.professor {
            Some(p) => p,
            None => continue,
        };

        let (lastname, firstname) = {
            let mut parts = professor.splitn(2, " ");
            (parts.next().unwrap(), parts.next().unwrap())
        };

        occupancies.push(NewOccupancySeed {
            classroom_name: event.location,
            group_number: match event.event_type {
                EventType::CM | EventType::Projet => None,
                EventType::TD | EventType::TP => Some(0),
            },
            start_datetime: event.start as u64,
            end_datetime: event.end as u64,
            occupancy_type: match event.event_type {
                EventType::CM => OccupancyType::CM,
                EventType::Projet => OccupancyType::Projet,
                EventType::TD => OccupancyType::TD,
                EventType::TP => OccupancyType::TP,
            },
            subject_name: event.subject,
            teacher_first_name: firstname.to_string(),
            teacher_last_name: lastname.to_string(),
            name: event.name,
        });
    }

    db.seed(
        users
            .into_iter()
            .chain(teachers.into_iter())
            .chain(students.into_iter()),
        classrooms.into_iter(),
        classes.into_iter(),
        subjects.into_iter(),
        occupancies.into_iter(),
    );
}

fn test_users() -> Vec<NewUser> {
    vec![
        NewUser {
            first_name: "Admin".to_string(),
            last_name: "User".to_string(),
            password: "user.admin".to_string(),
            kind: UserKind::Administrator,
        },
        NewUser {
            first_name: "Teacher".to_string(),
            last_name: "User".to_string(),
            password: "user.teacher".to_string(),
            kind: UserKind::Teacher(TeacherInformations {
                phone_number: Some(random_phone_number(rand::thread_rng())),
                email: Some("teacher@edu.univ-amu.fr".to_string()),
                rank: Rank::Professor,
            }),
        },
        NewUser {
            first_name: "Student".to_string(),
            last_name: "User".to_string(),
            password: "user.student".to_string(),
            kind: UserKind::Student(StudentInformations {
                class_id: 0, // TODO
            }),
        },
    ]
}

fn test_classrooms(events: &Vec<Event>) -> Vec<NewClassroom> {
    events
        .iter()
        .map(|event| event.location.as_str())
        .unique()
        .map(|name| NewClassroom {
            name: name.to_string(),
            capacity: 50,
        })
        .collect()
}

fn test_teachers(events: &Vec<Event>) -> Vec<NewUser> {
    let teachers: Vec<&String> = events
        .iter()
        .filter_map(|e| e.professor.as_ref())
        .unique()
        .collect();

    let rng = rand::thread_rng();
    let mut new_users = Vec::new();

    for teacher_name in teachers {
        let (lastname, firstname) = {
            let mut parts = teacher_name.splitn(2, " ");
            (parts.next().unwrap(), parts.next().unwrap())
        };

        let username = username_from_name(firstname, lastname);

        let informations = TeacherInformations {
            phone_number: Some(random_phone_number(rng)),
            email: Some(format!("{}@edu.univ-amu.fr", username)),
            rank: Rank::Professor,
        };

        new_users.push(NewUser {
            first_name: firstname.to_string(),
            last_name: lastname.to_string(),
            password: username,
            kind: UserKind::Teacher(informations),
        });
    }

    new_users
}

/// Generates a random french mobile phone number, with a prefix of 0[6-7]
fn random_phone_number(mut rng: impl Rng) -> String {
    (0..10)
        .map(|i| {
            format!(
                "{}",
                if i == 0 {
                    0
                } else if i == 1 {
                    rng.gen_range(6, 8)
                } else {
                    rng.gen_range(0, 10)
                }
            )
        })
        .collect()
}

fn test_classes() -> Vec<NewClass> {
    vec![NewClass {
        name: "L3 Informatique".to_string(),
        level: ClassLevel::L3,
    }]
}

fn test_students(names: &Vec<StudentName>) -> Vec<NewUser> {
    let mut new_users = Vec::new();

    for name in names {
        let username = username_from_name(&name.first_name, &name.last_name);

        let informations = StudentInformations {
            class_id: 0, // TODO
        };

        new_users.push(NewUser {
            first_name: name.first_name.clone(),
            last_name: name.last_name.clone(),
            password: username,
            kind: UserKind::Student(informations),
        });
    }

    new_users
}

fn test_subjects(events: &Vec<Event>) -> Vec<NewSubject> {
    events
        .iter()
        .map(|e| e.subject.to_string())
        .unique()
        .enumerate()
        .map(|(index, name)| NewSubject {
            name,
            class_id: 0,                            // TODO
            teacher_in_charge_id: 3 + index as u32, // TODO
        })
        .collect()
}
