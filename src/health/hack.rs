use once_cell::sync::Lazy;
use rocket::route::Route;

#[rocket::get("/")]
fn f() {}

fn do_get() -> Route {
    let v = rocket::routes!(f);
    v.into_iter().next().unwrap()
}

static CACHE: Lazy<Route> = Lazy::new(do_get);

pub(crate) fn get() -> Route {
    (*CACHE).clone()
}
