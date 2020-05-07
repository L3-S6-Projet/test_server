use log;
use serde::{Deserialize, Serialize};
use warp::{http::StatusCode, Filter, Rejection, Reply};

use super::{
    globals::{
        OccupanciesListResponse, OccupanciesRequest, PaginatedQueryableListRequest,
        SimpleSuccessResponse,
    },
    ErrorCode, FailureResponse,
};
use db::{
    group_numbers,
    models::{OccupancyType, UserKind},
    Database, Db, LockedDb, NewOccupancy, NewSubject, SubjectUpdate,
};
use filters::{authed_is_of_kind, delayed, with_db, PossibleUserKind};

pub fn routes(db: &Db) -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone {
    let list_route = warp::path!("api" / "subjects")
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
    let create_route = warp::path!("api" / "subjects")
        .and(warp::post())
        .and(authed_is_of_kind(db, &[PossibleUserKind::Administrator]))
        .and(with_db(db.clone()))
        .and(warp::body::content_length_limit(1024 * 16).and(warp::body::json()))
        .and_then(create)
        .and(delayed(db))
        .boxed();

    // TODO: deletion constraints
    let delete_route = warp::path!("api" / "subjects")
        .and(warp::delete())
        .and(authed_is_of_kind(db, &[PossibleUserKind::Administrator]))
        .and(with_db(db.clone()))
        .and(warp::body::content_length_limit(1024 * 16).and(warp::body::json()))
        .and_then(delete)
        .and(delayed(db))
        .boxed();

    let get_route = warp::path!("api" / "subjects" / u32)
        .and(warp::get())
        .and(with_db(db.clone()))
        .and_then(get)
        .and(delayed(db))
        .boxed();

    let update_route = warp::path!("api" / "subjects" / u32)
        .and(warp::put())
        .and(with_db(db.clone()))
        .and(warp::body::content_length_limit(1024 * 16).and(warp::body::json()))
        .and_then(update)
        .and(delayed(db))
        .boxed();

    let teacher_post_route = warp::path!("api" / "subjects" / u32 / "teachers")
        .and(warp::post())
        .and(with_db(db.clone()))
        .and(warp::body::content_length_limit(1024 * 16).and(warp::body::json()))
        .and_then(teacher_post)
        .and(delayed(db))
        .boxed();

    let teacher_delete_route = warp::path!("api" / "subjects" / u32 / "teachers")
        .and(warp::delete())
        .and(with_db(db.clone()))
        .and(warp::body::content_length_limit(1024 * 16).and(warp::body::json()))
        .and_then(teacher_delete)
        .and(delayed(db))
        .boxed();

    let group_post_route = warp::path!("api" / "subjects" / u32 / "groups")
        .and(warp::post())
        .and(with_db(db.clone()))
        .and_then(group_post)
        .and(delayed(db))
        .boxed();

    let group_delete_route = warp::path!("api" / "subjects" / u32 / "groups")
        .and(warp::delete())
        .and(with_db(db.clone()))
        .and_then(group_delete)
        .and(delayed(db))
        .boxed();

    let occupancies_get_route = warp::path!("api" / "subjects" / u32 / "occupancies")
        .and(warp::get())
        .and(with_db(db.clone()))
        .and(warp::query::<OccupanciesRequest>())
        .and_then(occupancies_get)
        .and(delayed(db))
        .boxed();

    // TODO: creation constraint (unique name?)
    let occupancies_create_route = warp::path!("api" / "subjects" / u32 / "occupancies")
        .and(warp::post())
        .and(authed_is_of_kind(db, &[PossibleUserKind::Administrator]))
        .and(with_db(db.clone()))
        .and(warp::body::content_length_limit(1024 * 16).and(warp::body::json()))
        .and_then(occupancies_create)
        .and(delayed(db))
        .boxed();

    // TODO: creation constraint (unique name?)
    let occupancies_groups_create_route =
        warp::path!("api" / "subjects" / u32 / "groups" / u32 / "occupancies")
            .and(warp::post())
            .and(authed_is_of_kind(db, &[PossibleUserKind::Administrator]))
            .and(with_db(db.clone()))
            .and(warp::body::content_length_limit(1024 * 16).and(warp::body::json()))
            .and_then(occupancies_groups_create)
            .and(delayed(db))
            .boxed();

    let occupancies_group_get_route =
        warp::path!("api" / "subjects" / u32 / "groups" / u32 / "occupancies")
            .and(warp::get())
            .and(authed_is_of_kind(db, &[PossibleUserKind::Administrator]))
            .and(with_db(db.clone()))
            .and(warp::query::<OccupanciesRequest>())
            .and_then(occupancies_group_get)
            .and(delayed(db))
            .boxed();

    list_route
        .or(create_route)
        .or(delete_route)
        .or(get_route)
        .or(update_route)
        .or(teacher_post_route)
        .or(teacher_delete_route)
        .or(group_post_route)
        .or(group_delete_route)
        .or(occupancies_get_route)
        .or(occupancies_create_route)
        .or(occupancies_groups_create_route)
        .or(occupancies_group_get_route)
}

#[derive(Serialize)]
struct ListResponse<'a> {
    status: &'static str,
    total: usize,
    subjects: Vec<ListResponseItem<'a>>, // TODO: remove group_count from here
}

#[derive(Serialize)]
struct ListResponseItem<'a> {
    id: u32,
    class_name: &'a str,
    name: &'a str,
}

async fn list(
    _username: String,
    db: Db,
    request: PaginatedQueryableListRequest,
) -> Result<impl warp::Reply, warp::Rejection> {
    let db = db.lock().await;

    let page = request.normalized_page_number();
    let (total, subjects) = db.subject_list(page, request.query.as_deref(), |_| true);

    let subjects = subjects
        .iter()
        .map(|s| ListResponseItem {
            id: s.id,
            class_name: &db
                .class_get(s.class_id)
                .expect("should be a valid reference")
                .name,
            name: &s.name,
        })
        .collect();

    Ok(warp::reply::json(&ListResponse {
        status: "success",
        total,
        subjects,
    }))
}

async fn create(
    _username: String,
    db: Db,
    request: NewSubject,
) -> Result<impl warp::Reply, warp::Rejection> {
    let mut db = db.lock().await;

    if db.class_get(request.class_id).is_none() {
        return Ok(warp::reply::with_status(
            warp::reply::json(&FailureResponse::new(ErrorCode::InvalidID)),
            StatusCode::NOT_FOUND,
        ));
    }

    db.subject_add(request);

    Ok(warp::reply::with_status(
        warp::reply::json(&SimpleSuccessResponse::new()),
        StatusCode::OK,
    ))
}

async fn delete(
    _username: String,
    db: Db,
    request: Vec<u32>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let mut db = db.lock().await;

    if db.subject_remove(&request) {
        Ok(warp::reply::with_status(
            warp::reply::json(&SimpleSuccessResponse::new()),
            StatusCode::OK,
        ))
    } else {
        Ok(warp::reply::with_status(
            warp::reply::json(&FailureResponse::new(ErrorCode::InvalidID)),
            StatusCode::NOT_FOUND,
        ))
    }
}

#[derive(Serialize)]
struct GetResponse<'a> {
    status: &'static str,
    subject: GetResponseSubject<'a>,
}

#[derive(Serialize)]
struct GetResponseSubject<'a> {
    name: &'a str,
    class_name: &'a str,
    total_hours: u32, // TODO
    teachers: Vec<GetResponseTeacher<'a>>,
    groups: Vec<GetResponseGroup>,
}

#[derive(Serialize)]
struct GetResponseTeacher<'a> {
    pub id: u32,
    pub first_name: &'a str,
    pub last_name: &'a str,
    pub in_charge: bool,
}

#[derive(Serialize)]
struct GetResponseGroup {
    pub id: u32,
    pub name: String,
    pub count: u32,
}

async fn get(id: u32, db: Db) -> Result<impl warp::Reply, std::convert::Infallible> {
    let db = db.lock().await;

    let total_student_count: usize = db.subject_students(id).len();

    let subject = match db.subject_get(id) {
        Some(u) => u,
        None => {
            return Ok(warp::reply::with_status(
                warp::reply::json(&FailureResponse::new(ErrorCode::InvalidID)),
                StatusCode::NOT_FOUND,
            ))
        }
    };

    let class = match db.class_get(subject.class_id) {
        Some(c) => c,
        None => panic!("the class reference should be valid"),
    };

    let teachers: Vec<GetResponseTeacher> = db
        .user_list(None, None, |u| match u.kind {
            UserKind::Student(_) => false,
            UserKind::Administrator => false,
            UserKind::Teacher(_) => true,
        })
        .1
        .iter()
        .filter(|teacher| db.teacher_teaches(teacher.id, subject.id))
        .map(|user| GetResponseTeacher {
            id: user.id,
            first_name: &user.first_name,
            last_name: &user.last_name,
            in_charge: db.teacher_in_charge(user.id, id),
        })
        .collect();

    let groups: Vec<GetResponseGroup> = (0..subject.group_count)
        .zip(group_numbers(total_student_count, subject.group_count))
        .map(|(number, group_count)| GetResponseGroup {
            id: number,
            name: format!("Groupe {}", number + 1),
            count: group_count,
        })
        .collect();

    Ok(warp::reply::with_status(
        warp::reply::json(&GetResponse {
            status: "success",
            subject: GetResponseSubject {
                name: &subject.name,
                class_name: &class.name,
                total_hours: 0, // TODO
                teachers,
                groups,
            },
        }),
        StatusCode::OK,
    ))
}

async fn update(
    id: u32,
    db: Db,
    request: SubjectUpdate,
) -> Result<impl warp::Reply, std::convert::Infallible> {
    let mut db = db.lock().await;

    // First: validate teacher already teaches that subject
    if let Some(teacher_id) = request.teacher_in_charge_id {
        if !db.teacher_teaches(teacher_id, id) {
            return Ok(warp::reply::with_status(
                warp::reply::json(&FailureResponse::new(ErrorCode::InvalidID)),
                StatusCode::UNPROCESSABLE_ENTITY,
            ));
        }
    }

    // Then: validate class id is valid
    if let Some(class_id) = request.class_id {
        if db.class_get(class_id).is_none() {
            return Ok(warp::reply::with_status(
                warp::reply::json(&FailureResponse::new(ErrorCode::InvalidID)),
                StatusCode::UNPROCESSABLE_ENTITY,
            ));
        }
    }

    let status = db.subject_update(id, request);

    if status.found {
        Ok(warp::reply::with_status(
            warp::reply::json(&SimpleSuccessResponse::new()),
            if status.updated {
                StatusCode::OK
            } else {
                StatusCode::NO_CONTENT
            },
        ))
    } else {
        Ok(warp::reply::with_status(
            warp::reply::json(&FailureResponse::new(ErrorCode::InvalidID)),
            StatusCode::NOT_FOUND,
        ))
    }
}

async fn teacher_post(
    subject_id: u32,
    db: Db,
    request: Vec<u32>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let mut db = db.lock().await;
    let subject = db.subject_get(subject_id);

    if subject.is_none() {
        return Ok(warp::reply::with_status(
            warp::reply::json(&FailureResponse::new(ErrorCode::InvalidID)),
            StatusCode::NOT_FOUND,
        ));
    }

    let all_teachers_exist = request
        .iter()
        .all(|id| db.user_get_teacher_by_id(*id).is_some());

    if !all_teachers_exist {
        return Ok(warp::reply::with_status(
            warp::reply::json(&FailureResponse::new(ErrorCode::InvalidID)),
            StatusCode::NOT_FOUND,
        ));
    }

    for id in &request {
        db.teacher_set_teaches(*id, subject_id);
    }

    Ok(warp::reply::with_status(
        warp::reply::json(&SimpleSuccessResponse::new()),
        if request.len() > 0 {
            StatusCode::OK
        } else {
            StatusCode::NO_CONTENT
        },
    ))
}

async fn teacher_delete(
    subject_id: u32,
    db: Db,
    request: Vec<u32>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let mut db = db.lock().await;
    let subject = db.subject_get(subject_id);

    if subject.is_none() {
        return Ok(warp::reply::with_status(
            warp::reply::json(&FailureResponse::new(ErrorCode::InvalidID)),
            StatusCode::NOT_FOUND,
        ));
    }

    let all_teachers_exist_and_teaching_subject_but_not_in_charge = request.iter().all(|id| {
        db.user_get_teacher_by_id(*id).is_some()
            && db.teacher_teaches(*id, subject_id)
            && !db.teacher_in_charge(*id, subject_id)
    });

    if !all_teachers_exist_and_teaching_subject_but_not_in_charge {
        return Ok(warp::reply::with_status(
            warp::reply::json(&FailureResponse::new(ErrorCode::InvalidID)),
            StatusCode::NOT_FOUND,
        ));
    }

    // Should not be needed, because there will always be at least one teacher in charge (checked above)
    let count_after_deletion = db
        .user_list(None, None, |u| match u.kind {
            UserKind::Student(_) => false,
            UserKind::Administrator => false,
            UserKind::Teacher(_) => true,
        })
        .1
        .iter()
        .filter(|teacher| db.teacher_teaches(teacher.id, subject_id))
        .filter(|teacher| !request.contains(&teacher.id))
        .count();

    if count_after_deletion == 0 {
        return Ok(warp::reply::with_status(
            warp::reply::json(&FailureResponse::new(ErrorCode::IllegalRequest)),
            StatusCode::NOT_FOUND,
        ));
    }

    for id in &request {
        db.teacher_unset_teaches(*id, subject_id);
    }

    Ok(warp::reply::with_status(
        warp::reply::json(&SimpleSuccessResponse::new()),
        if request.len() > 0 {
            StatusCode::OK
        } else {
            StatusCode::NO_CONTENT
        },
    ))
}

async fn group_post(subject_id: u32, db: Db) -> Result<impl warp::Reply, warp::Rejection> {
    // Set group_count and group_number
    let mut db = db.lock().await;

    let subject = db.subject_get(subject_id);

    if subject.is_none() {
        return Ok(warp::reply::with_status(
            warp::reply::json(&FailureResponse::new(ErrorCode::InvalidID)),
            StatusCode::NOT_FOUND,
        ));
    }

    db.subject_add_group(subject_id);
    db.distribute_subject_groups(subject_id);

    Ok(warp::reply::with_status(
        warp::reply::json(&SimpleSuccessResponse::new()),
        StatusCode::OK,
    ))
}

async fn group_delete(subject_id: u32, db: Db) -> Result<impl warp::Reply, warp::Rejection> {
    // Set group_count and group_number
    let mut db = db.lock().await;

    let subject = db.subject_get(subject_id);

    if subject.is_none() {
        return Ok(warp::reply::with_status(
            warp::reply::json(&FailureResponse::new(ErrorCode::InvalidID)),
            StatusCode::NOT_FOUND,
        ));
    }

    if !db.subject_remove_group(subject_id) {
        return Ok(warp::reply::with_status(
            warp::reply::json(&FailureResponse::new(ErrorCode::IllegalRequest)),
            StatusCode::NOT_FOUND,
        ));
    }

    db.distribute_subject_groups(subject_id);

    Ok(warp::reply::with_status(
        warp::reply::json(&SimpleSuccessResponse::new()),
        StatusCode::OK,
    ))
}

async fn occupancies_get(
    subject_id: u32,
    db: Db,
    request: OccupanciesRequest,
) -> Result<impl warp::Reply, warp::Rejection> {
    let db = db.lock().await;

    if db.subject_get(subject_id).is_none() {
        return Ok(warp::reply::with_status(
            warp::reply::json(&FailureResponse::new(ErrorCode::InvalidID)),
            StatusCode::NOT_FOUND,
        ));
    }

    let occupancies_list = db.occupancies_list(request.start, request.end);

    let occupancies_list = occupancies_list
        .into_iter()
        .filter(|o| o.subject_id == Some(subject_id))
        .collect();

    let response =
        OccupanciesListResponse::from_list(&db, occupancies_list, request.occupancies_per_day);

    Ok(warp::reply::with_status(
        warp::reply::json(&response),
        StatusCode::OK,
    ))
}

#[derive(Deserialize)]
struct SubjectOccupancyCreationRequest {
    pub classroom_id: Option<u32>,
    pub teacher_id: u32,
    pub start: u64,
    pub end: u64,
    pub occupancy_type: OccupancyType,
    pub name: String,
}

async fn occupancies_create(
    subject_id: u32,
    _username: String,
    db: Db,
    request: SubjectOccupancyCreationRequest,
) -> Result<impl warp::Reply, warp::Rejection> {
    let mut db = db.lock().await;

    if let Some(err_response) = validate_new_occupancy_base(&db, subject_id, &request) {
        return Ok(err_response);
    }

    // Type constraints
    match request.occupancy_type {
        OccupancyType::TD | OccupancyType::TP => {
            log::warn!("Trying to create an occupancy without a group, but the occupancy type is TD or TP. Specify a group to create those.");

            return Ok(warp::reply::with_status(
                warp::reply::json(&FailureResponse::new(ErrorCode::IllegalOccupancyType)),
                StatusCode::UNPROCESSABLE_ENTITY,
            ));
        }
        OccupancyType::CM | OccupancyType::Projet => {
            if request.classroom_id.is_none() {
                log::warn!("Trying to create an occupancy, and the type is CM or Projet, but the classroom id is not defined.");

                return Ok(warp::reply::with_status(
                    warp::reply::json(&FailureResponse::new(ErrorCode::InvalidID)),
                    StatusCode::NOT_FOUND,
                ));
            }
        }
        OccupancyType::Administration => {}
        OccupancyType::External => {}
    }

    // TODO: check request.occupancy_type parameters satisfied

    let occupancy = NewOccupancy {
        classroom_id: request.classroom_id,
        group_number: None,
        subject_id: Some(subject_id),
        teacher_id: request.teacher_id,
        start_datetime: request.start,
        end_datetime: request.end,
        occupancy_type: request.occupancy_type,
        name: request.name,
    };

    db.occupancies_add(occupancy);

    Ok(warp::reply::with_status(
        warp::reply::json(&SimpleSuccessResponse::new()),
        StatusCode::OK,
    ))
}

async fn occupancies_groups_create(
    subject_id: u32,
    group_number: u32,
    _username: String,
    db: Db,
    request: SubjectOccupancyCreationRequest,
) -> Result<impl warp::Reply, warp::Rejection> {
    let mut db = db.lock().await;

    if let Some(err_response) = validate_new_occupancy_base(&db, subject_id, &request) {
        return Ok(err_response);
    }

    // Type constraints
    match request.occupancy_type {
        OccupancyType::TD | OccupancyType::TP => {}
        _ => {
            log::warn!(
                "Trying to create an occupancy with a group, but the type is neither TD nor TP."
            );

            return Ok(warp::reply::with_status(
                warp::reply::json(&FailureResponse::new(ErrorCode::IllegalOccupancyType)),
                StatusCode::UNPROCESSABLE_ENTITY,
            ));
        }
    }

    if request.classroom_id.is_none() {
        log::warn!("Trying to create an occupancy, and the type is TD or TP, but the classroom id is not defined.");

        return Ok(warp::reply::with_status(
            warp::reply::json(&FailureResponse::new(ErrorCode::InvalidID)),
            StatusCode::NOT_FOUND,
        ));
    }

    let subject = db
        .subject_get(subject_id)
        .expect("should be a valid id, since its already been validated");

    if group_number >= subject.group_count {
        log::warn!("Trying to create an occupancy, but the provided group number is invalid.");

        return Ok(warp::reply::with_status(
            warp::reply::json(&FailureResponse::new(ErrorCode::InvalidID)),
            StatusCode::NOT_FOUND,
        ));
    }

    // TODO: check that group number is valid

    let occupancy = NewOccupancy {
        classroom_id: request.classroom_id,
        group_number: Some(group_number),
        subject_id: Some(subject_id),
        teacher_id: request.teacher_id,
        start_datetime: request.start,
        end_datetime: request.end,
        occupancy_type: request.occupancy_type,
        name: request.name,
    };

    db.occupancies_add(occupancy);

    Ok(warp::reply::with_status(
        warp::reply::json(&SimpleSuccessResponse::new()),
        StatusCode::OK,
    ))
}

fn validate_new_occupancy_base(
    db: &LockedDb,
    subject_id: u32,
    request: &SubjectOccupancyCreationRequest,
) -> Option<warp::reply::WithStatus<warp::reply::Json>> {
    // Check subject exists
    if db.subject_get(subject_id).is_none() {
        log::warn!("Trying to create an occupancy but the subject does not exist.");

        return Some(warp::reply::with_status(
            warp::reply::json(&FailureResponse::new(ErrorCode::InvalidID)),
            StatusCode::NOT_FOUND,
        ));
    }

    // Check teacher exists and teaches that subject
    match db.user_get_teacher_by_id(request.teacher_id) {
        Some(_) => {
            if !db.teacher_teaches(request.teacher_id, subject_id) {
                log::warn!(
                    "Trying to create an occupancy but the teacher does not teach that subject."
                );
                return Some(warp::reply::with_status(
                    warp::reply::json(&FailureResponse::new(ErrorCode::TeacherDoesNotTeach)),
                    StatusCode::UNPROCESSABLE_ENTITY,
                ));
            }
        }
        None => {
            log::warn!("Trying to create an occupancy but the teacher does not exist.");

            return Some(warp::reply::with_status(
                warp::reply::json(&FailureResponse::new(ErrorCode::InvalidID)),
                StatusCode::NOT_FOUND,
            ));
        }
    }

    // Check that classroom exists, and that is is free
    if let Some(classroom_id) = request.classroom_id {
        if db.classroom_get(classroom_id).is_none() {
            log::warn!("Trying to create an occupancy but the classroom does not exist.");

            return Some(warp::reply::with_status(
                warp::reply::json(&FailureResponse::new(ErrorCode::InvalidID)),
                StatusCode::NOT_FOUND,
            ));
        }

        if !db.classroom_free(classroom_id, request.start, request.end) {
            log::warn!("Trying to create an occupancy but the classroom is not free.");

            return Some(warp::reply::with_status(
                warp::reply::json(&FailureResponse::new(ErrorCode::ClassroomAlreadyOccupied)),
                StatusCode::UNPROCESSABLE_ENTITY,
            ));
        }
    }

    // Check end_datetime >= start_datetime
    if request.end < request.start {
        log::warn!(
            "Trying to create an occupancy but the end_datetime is before the start_datetime."
        );

        return Some(warp::reply::with_status(
            warp::reply::json(&FailureResponse::new(ErrorCode::EndBeforeStart)),
            StatusCode::UNPROCESSABLE_ENTITY,
        ));
    }

    // Check that the teacher is free
    if !db.teacher_free(request.teacher_id, request.start, request.end) {
        log::warn!("Trying to create an occupancy, but the teacher is not free.");

        return Some(warp::reply::with_status(
            warp::reply::json(&FailureResponse::new(ErrorCode::Unknown)),
            StatusCode::UNPROCESSABLE_ENTITY,
        ));
    }

    // TODO: Check that the class is free

    None
}

async fn occupancies_group_get(
    subject_id: u32,
    group_number: u32,
    _username: String,
    db: Db,
    request: OccupanciesRequest,
) -> Result<impl warp::Reply, warp::Rejection> {
    let db = db.lock().await;

    if db.subject_get(subject_id).is_none() {
        return Ok(warp::reply::with_status(
            warp::reply::json(&FailureResponse::new(ErrorCode::InvalidID)),
            StatusCode::NOT_FOUND,
        ));
    }

    let occupancies_list = db.occupancies_list(request.start, request.end);

    let occupancies_list = occupancies_list
        .into_iter()
        .filter(|o| o.subject_id == Some(subject_id) && o.group_number == Some(group_number))
        .collect();

    let response =
        OccupanciesListResponse::from_list(&db, occupancies_list, request.occupancies_per_day);

    Ok(warp::reply::with_status(
        warp::reply::json(&response),
        StatusCode::OK,
    ))
}
