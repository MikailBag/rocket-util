use rocket::State;

struct Approved(rocket_util::health::Condition);

#[rocket::post("/approve")]
async fn approve(cond: &State<Approved>) {
    cond.0.report_ok().await;
}

#[rocket::launch]
async fn rocket() -> _ {
    let (handle, route) = rocket_util::health::make();

    rocket::build()
        .manage(Approved(handle.condition("approved").await))
        .mount("/health", [route])
        .mount("/", rocket::routes!(approve))
}
