mod hack;

use rocket::{http::Status, route::Outcome, Response, Route};
use serde::Serialize;
use std::{collections::HashMap, io::Cursor, sync::Arc};
use tokio::sync::Mutex;

/// Manages one kind of health (such as readiness
/// or liveness)
#[derive(Clone)]
pub struct Health {
    inner: Arc<Mutex<Inner>>,
}

impl Health {
    pub async fn condition(&self, name: &str) -> Condition {
        let c = Condition {
            name: name.into(),
            inner: self.inner.clone(),
        };
        // initially condition is false
        c.report_failure().await;

        c
    }
}

/// Handle for manipulating particular health condition
#[derive(Clone)]
pub struct Condition {
    inner: Arc<Mutex<Inner>>,
    name: Arc<str>,
}

impl Condition {
    async fn put(&self, v: bool) {
        let mut inner = self.inner.lock().await;
        if let Some(u) = inner.checks.get_mut(&*self.name) {
            *u = v;
        } else {
            inner.checks.insert(self.name.to_string(), v);
        }
    }

    pub async fn report_ok(&self) {
        self.put(true).await;
    }

    pub async fn report_failure(&self) {
        self.put(false).await;
    }
}

/// Route handler
#[derive(Clone)]
struct Handler {
    inner: Arc<Mutex<Inner>>,
}

#[derive(Serialize)]
struct HealthInfo {
    ok: bool,
    failing_checks: Vec<String>,
}

struct Inner {
    checks: HashMap<String, bool>,
}

impl Inner {
    fn describe(&self) -> HealthInfo {
        let ok = self.checks.values().all(|x| *x);
        let failing_checks = self
            .checks
            .iter()
            .filter(|(_, v)| **v)
            .map(|(k, _)| k.clone())
            .collect();
        HealthInfo { ok, failing_checks }
    }
}

#[rocket::async_trait]
impl rocket::route::Handler for Handler {
    async fn handle<'r>(
        &self,
        _request: &'r rocket::Request<'_>,
        _data: rocket::Data,
    ) -> Outcome<'r> {
        let inner = self.inner.lock().await;
        let h = inner.describe();
        let h = match serde_json::to_vec(&h) {
            Ok(h) => h,
            Err(_) => {
                return Outcome::Failure(Status::InternalServerError);
            }
        };
        let resp = Response::build()
            .sized_body(h.len(), Cursor::new(h))
            .finalize();
        Outcome::Success(resp)
    }
}

/// Creates a route for particular kind of help
/// and a handle to manage this state
pub fn make() -> (Health, Route) {
    let inner = Inner {
        checks: HashMap::new(),
    };
    let inner = Arc::new(Mutex::new(inner));

    let handle = Health {
        inner: inner.clone(),
    };
    let handler = Handler { inner };

    let mut route = hack::get();

    route.name = None;
    route.method = rocket::http::Method::Get;
    route.handler = Box::new(handler);
    route.rank = 0;
    route.format = Some(rocket::http::MediaType::JSON);

    (handle, route)
}
