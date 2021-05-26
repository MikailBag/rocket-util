use rocket::State;

struct Approved(rocket_util::health::Condition);

#[rocket::post("/approve")]
async fn approve(cond: &State<Approved>, _auth: rocket_util::authn::UserInfo) {
    cond.0.report_ok().await;
}

#[rocket::get("/")]
async fn say_hi() -> &'static str {
    "hi there"
}

#[rocket::launch]
async fn rocket() -> _ {
    let (handle, route) = rocket_util::health::make("health");

    rocket::build()
        .manage(Approved(handle.condition("approved").await))
        .manage(rocket_util::authn::AuthentifierConfig {
            request_header: Some(rocket_util::authn::RequestHeaderAuthentifierConfig {
                username: "X-User".to_string(),
                group: "X-Group".to_string()
            })
        })
        .mount("/", [route])
        .mount("/", rocket::routes!(approve, say_hi))
}
