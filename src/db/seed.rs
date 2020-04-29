use super::{
    models::{Classroom, Rank, StudentInformations, TeacherInformations, UserKind},
    Database, NewUser,
};
use crate::assets::Event;
use crate::utils::UniqueExt;
use rand::{self, Rng};

pub fn seed_db<D: Database>(db: &mut D) {
    let events = Event::from_parsed_ical();

    let users = test_users();
    let classrooms = test_classrooms(&events);
    let teachers = test_teachers(&events);

    db.seed(
        users.into_iter().chain(teachers.into_iter()),
        classrooms.into_iter(),
    );
}

fn test_users() -> Vec<NewUser> {
    vec![
        NewUser {
            first_name: "Admin".to_string(),
            last_name: "User".to_string(),
            username: "admin".to_string(),
            password: "admin".to_string(),
            kind: UserKind::Administrator,
        },
        NewUser {
            first_name: "Professor".to_string(),
            last_name: "User".to_string(),
            username: "professor".to_string(),
            password: "professor".to_string(),
            kind: UserKind::Teacher(TeacherInformations {
                phone_number: random_phone_number(rand::thread_rng()),
                email: "professor@edu.univ-amu.fr".to_string(),
                rank: Rank::Professor,
            }),
        },
        NewUser {
            first_name: "Student".to_string(),
            last_name: "User".to_string(),
            username: "student".to_string(),
            password: "student".to_string(),
            kind: UserKind::Student(StudentInformations {
                class_id: 0, // TODO
            }),
        },
    ]
}

fn test_classrooms(events: &Vec<Event>) -> Vec<Classroom> {
    events
        .iter()
        .map(|event| event.location.as_str())
        .unique()
        .map(|name| Classroom {
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

        let username = unidecode::unidecode(teacher_name)
            .to_ascii_lowercase()
            .replace(" ", ".");

        let informations = TeacherInformations {
            phone_number: random_phone_number(rng),
            email: format!("{}@edu.univ-amu.fr", username),
            rank: Rank::Professor,
        };

        new_users.push(NewUser {
            first_name: firstname.to_string(),
            last_name: lastname.to_string(),
            username: username.clone(),
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
