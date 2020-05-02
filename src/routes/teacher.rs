use rand::{self, distributions::Alphanumeric, Rng};
use serde::{Deserialize, Serialize};
use warp::{http::StatusCode, Filter, Rejection, Reply};

use super::{
    globals::{
        deserialize_some, AccountCreatedResponse, PaginatedQueryableListRequest,
        SimpleSuccessResponse,
    },
    ErrorCode, FailureResponse,
};
use db::{
    group_numbers,
    models::{Rank, TeacherInformations, UserKind},
    Database, Db, NewUser,
};
use filters::{authed_is_of_kind, delayed, with_db, PossibleUserKind};

pub fn routes(db: &Db) -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone {
    let list_route = warp::path!("api" / "teachers")
        .and(warp::get())
        .and(authed_is_of_kind(
            db,
            &[PossibleUserKind::Administrator, PossibleUserKind::Teacher],
        ))
        .and(with_db(db.clone()))
        .and(warp::query::<PaginatedQueryableListRequest>())
        .and_then(list)
        .and(delayed(db))
        .boxed();

    // TODO: creation constraint (unique name?)
    let create_route = warp::path!("api" / "teachers")
        .and(warp::post())
        .and(authed_is_of_kind(db, &[PossibleUserKind::Administrator]))
        .and(with_db(db.clone()))
        .and(warp::body::content_length_limit(1024 * 16).and(warp::body::json()))
        .and_then(create)
        .and(delayed(db))
        .boxed();

    // TODO: deletion constraints
    let delete_route = warp::path!("api" / "teachers")
        .and(warp::delete())
        .and(authed_is_of_kind(db, &[PossibleUserKind::Administrator]))
        .and(with_db(db.clone()))
        .and(warp::body::content_length_limit(1024 * 16).and(warp::body::json()))
        .and_then(delete)
        .and(delayed(db))
        .boxed();

    // TODO: missing auth??

    let get_route = warp::path!("api" / "teachers" / u32)
        .and(warp::get())
        .and(with_db(db.clone()))
        .and_then(get)
        .and(delayed(db))
        .boxed();

    let update_route = warp::path!("api" / "teachers" / u32)
        .and(warp::put())
        .and(with_db(db.clone()))
        .and(warp::body::content_length_limit(1024 * 16).and(warp::body::json()))
        .and_then(update)
        .and(delayed(db))
        .boxed();

    let subjects_get_route = warp::path!("api" / "teachers" / u32 / "subjects")
        .and(warp::get())
        .and(with_db(db.clone()))
        .and_then(subjects_get)
        .and(delayed(db))
        .boxed();

    list_route
        .or(create_route)
        .or(delete_route)
        .or(get_route)
        .or(update_route)
        .or(subjects_get_route)
}

#[derive(Serialize)]
struct ListResponse<'a> {
    status: &'static str,
    total: usize,
    teachers: Vec<Teacher<'a>>,
}

#[derive(Serialize)]
struct Teacher<'a> {
    id: u32,
    first_name: &'a str,
    last_name: &'a str,
    email: Option<&'a str>,
    phone_number: Option<&'a str>,
}

async fn list(
    _username: String,
    db: Db,
    request: PaginatedQueryableListRequest,
) -> Result<impl warp::Reply, warp::Rejection> {
    let db = db.lock().await;

    let page = request.normalized_page_number();
    let (total, users) = db.user_list(Some(page), request.query.as_deref(), |u| match u.kind {
        UserKind::Student(_) => false,
        UserKind::Administrator => false,
        UserKind::Teacher(_) => true,
    });

    let teachers = users
        .into_iter()
        .map(|u| match &u.kind {
            UserKind::Teacher(informations) => Teacher {
                id: u.id,
                first_name: &u.first_name,
                last_name: &u.last_name,
                email: informations.email.as_deref(),
                phone_number: informations.phone_number.as_deref(),
            },
            UserKind::Administrator => unreachable!(),
            UserKind::Student(_) => unreachable!(),
        })
        .collect();

    Ok(warp::reply::json(&ListResponse {
        status: "success",
        total,
        teachers,
    }))
}

#[derive(Deserialize)]
struct NewTeacher {
    first_name: String,
    last_name: String,
    email: Option<String>,
    phone_number: Option<String>,
    rank: Rank,
}

async fn create(
    _username: String,
    db: Db,
    request: NewTeacher,
) -> Result<impl warp::Reply, warp::Rejection> {
    let mut db = db.lock().await;

    let mut rng = rand::thread_rng();

    let password = std::iter::repeat(())
        .map(|()| rng.sample(Alphanumeric))
        .take(10)
        .collect();

    let user = NewUser {
        first_name: request.first_name,
        last_name: request.last_name,
        password,
        kind: UserKind::Teacher(TeacherInformations {
            phone_number: request.phone_number,
            email: request.email,
            rank: request.rank,
        }),
    };

    let user = db.user_add(user);

    Ok(warp::reply::json(&AccountCreatedResponse {
        status: "success",
        username: &user.username,
        password: &user.password,
    }))
}

async fn delete(
    _username: String,
    db: Db,
    request: Vec<u32>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let mut db = db.lock().await;

    let all_exist_and_teacher =
        request
            .iter()
            .map(|id| db.user_get_by_id(*id))
            .all(|user| match user.map(|u| &u.kind) {
                Some(UserKind::Teacher(_)) => true,
                Some(UserKind::Administrator) | Some(UserKind::Student(_)) | None => false,
            });

    if !all_exist_and_teacher {
        return Ok(warp::reply::with_status(
            warp::reply::json(&FailureResponse::new(ErrorCode::InvalidID)),
            StatusCode::NOT_FOUND,
        ));
    }

    if db.user_remove(&request) {
        Ok(warp::reply::with_status(
            warp::reply::json(&SimpleSuccessResponse::new()),
            StatusCode::OK,
        ))
    } else {
        unreachable!("Since we checked that the users exist, they should be able to be removed")
    }
}

#[derive(Serialize)]
struct GetResponse<'a> {
    status: &'static str,
    teacher: GetResponseTeacher<'a>,
}

#[derive(Serialize)]
struct GetResponseTeacher<'a> {
    first_name: &'a str,
    last_name: &'a str,
    username: &'a str,
    email: Option<&'a str>,
    phone_number: Option<&'a str>,
    rank: &'a Rank,
    //total_service: u32, // TODO: total_service
    // TODO: services
}

async fn get(id: u32, db: Db) -> Result<impl warp::Reply, std::convert::Infallible> {
    let db = db.lock().await;
    let user = db.user_get_by_id(id);

    let res_teacher = match user {
        Some(user) => match &user.kind {
            UserKind::Administrator => None,
            UserKind::Teacher(informations) => Some(GetResponseTeacher {
                first_name: &user.first_name,
                last_name: &user.last_name,
                username: &user.username,
                email: informations.email.as_deref(),
                phone_number: informations.phone_number.as_deref(),
                rank: &informations.rank,
            }),
            UserKind::Student(_) => None,
        },
        None => None,
    };

    match res_teacher {
        Some(res_teacher) => Ok(warp::reply::with_status(
            warp::reply::json(&GetResponse {
                status: "success",
                teacher: res_teacher,
            }),
            StatusCode::OK,
        )),
        None => Ok(warp::reply::with_status(
            warp::reply::json(&FailureResponse::new(ErrorCode::InvalidID)),
            StatusCode::NOT_FOUND,
        )),
    }
}

#[derive(Deserialize, Debug)]
struct TeacherUpdate {
    first_name: Option<String>,
    last_name: Option<String>,
    #[serde(default, deserialize_with = "deserialize_some")]
    email: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_some")]
    phone_number: Option<Option<String>>,
    rank: Option<Rank>,
    password: Option<String>,
}

async fn update(
    id: u32,
    db: Db,
    request: TeacherUpdate,
) -> Result<impl warp::Reply, std::convert::Infallible> {
    let mut db = db.lock().await;

    let mut user = match db.user_get_teacher_by_id(id) {
        Some(user) => user,
        None => {
            return Ok(warp::reply::with_status(
                warp::reply::json(&FailureResponse::new(ErrorCode::InvalidID)),
                StatusCode::NOT_FOUND,
            ))
        }
    }
    .clone();

    let mut updated = false;

    macro_rules! update {
        ($obj:ident, $property:ident) => {
            if let Some(value) = request.$property {
                $obj.$property = value;
                updated = true;
            }
        };
    }

    update!(user, first_name);
    update!(user, last_name);
    update!(user, password);

    let mut informations = match &mut user.kind {
        UserKind::Administrator => unreachable!(),
        UserKind::Teacher(infos) => infos,
        UserKind::Student(_) => unreachable!(),
    };

    update!(informations, email);
    update!(informations, phone_number);
    update!(informations, rank);

    if updated {
        db.user_update(user);
    }

    Ok(warp::reply::with_status(
        warp::reply::json(&SimpleSuccessResponse::new()),
        if updated {
            StatusCode::OK
        } else {
            StatusCode::NO_CONTENT
        },
    ))
}

#[derive(Serialize)]
struct SubjectGetResponseList<'a> {
    status: &'static str,
    subjects: Vec<SubjectGetResponse<'a>>,
}

#[derive(Serialize)]
struct SubjectGetResponse<'a> {
    id: u32,
    name: &'a str,
    class_name: String,
    teachers: Vec<TeacherSubjectGetResponse<'a>>,
    groups: Vec<GroupSubjectGetResponse>,
}

#[derive(Serialize)]
struct TeacherSubjectGetResponse<'a> {
    first_name: &'a str,
    last_name: &'a str,
    in_charge: bool,
    email: Option<&'a str>,
    phone_number: Option<&'a str>,
}

#[derive(Serialize)]
struct GroupSubjectGetResponse {
    number: u32,
    name: String,
    count: u32,
}

async fn subjects_get(id: u32, db: Db) -> Result<impl warp::Reply, std::convert::Infallible> {
    let db = db.lock().await;

    // in: $teacher_id
    // list of all subjects $teacher_id participates in : db.teacher_subjects
    //    -> for each subject, list of all teachers : filter(db.teacher_teaches) as in subject.rs
    //    -> for each subject, list of all groups : just use subject.group_count + db::group_numbers as in subject.rs

    if db.user_get_teacher_by_id(id).is_none() {
        return Ok(warp::reply::with_status(
            warp::reply::json(&FailureResponse::new(ErrorCode::InvalidID)),
            StatusCode::NOT_FOUND,
        ));
    }

    let teacher_subjects = db.teacher_subjects(id);

    let mut subjects: Vec<SubjectGetResponse> = Vec::new();

    // For each subject that the teacher is in.
    for teacher_subject in teacher_subjects {
        // Eg: L3 Informatique
        let class_name = db
            .class_get(teacher_subject.class_id)
            .expect("invalid class_id in user informations")
            .name
            .to_string();

        // List of all teachers that teach this subject.
        let subject_teachers: Vec<TeacherSubjectGetResponse> = db.user_list(None, None, |u| match u.kind {
            UserKind::Student(_) => false,
            UserKind::Administrator => false,
            UserKind::Teacher(_) => true,
        } && db.teacher_teaches(u.id, teacher_subject.id)).1.iter().map(|u| {
            let informations = match &u.kind {
                UserKind::Student(_) => unreachable!(),
                UserKind::Administrator => unreachable!(),
                UserKind::Teacher(informations) => informations,
            };

            TeacherSubjectGetResponse {
                first_name: &u.first_name,
                last_name: &u.last_name,
                in_charge: db.teacher_in_charge(u.id, teacher_subject.id),
                email: informations.email.as_deref(),
                phone_number: informations.phone_number.as_deref(),
            }
        }).collect();

        let total_student_count: usize = db.subject_students(teacher_subject.id).len();

        let groups: Vec<GroupSubjectGetResponse> = (0..teacher_subject.group_count)
            .zip(group_numbers(
                total_student_count,
                teacher_subject.group_count,
            ))
            .map(|(number, group_count)| GroupSubjectGetResponse {
                number,
                name: format!("Groupe {}", number + 1),
                count: group_count,
            })
            .collect();

        subjects.push(SubjectGetResponse {
            id: teacher_subject.id,
            name: &teacher_subject.name,
            class_name,
            teachers: subject_teachers,
            groups,
        });
    }

    return Ok(warp::reply::with_status(
        warp::reply::json(&SubjectGetResponseList {
            status: "success",
            subjects,
        }),
        StatusCode::OK,
    ));
}
