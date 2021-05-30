use std::convert::Infallible;

use rocket::{
    http::{tls::ClientTls, Status},
    outcome::IntoOutcome,
    request::FromRequest,
    Request,
};
use thiserror::Error;
use x509_parser::x509::AttributeTypeAndValue;

#[derive(Debug, Error)]
pub enum Error {
    #[error("failed to parse certificate")]
    X509(#[from] x509_parser::nom::Err<x509_parser::error::X509Error>),
}

pub struct RequestHeaderAuthentifierConfig {
    pub username: String,
    pub group: String,
}

pub struct AuthentifierConfig {
    pub request_header: Option<RequestHeaderAuthentifierConfig>,
}

pub struct UserInfo {
    pub username: String,
    pub groups: Vec<String>,
}

impl UserInfo {
    pub fn member_of(&self, group: &str) -> bool {
        self.groups.iter().any(|g| g == group)
    }
}

fn parse_aval(av: &AttributeTypeAndValue<'_>) -> Option<String> {
    match av.as_str() {
        Ok(s) => Some(s.to_string()),
        Err(e) => {
            tracing::warn!(
                attribute_type_and_value = ?av,
                "ignoring unexpected AttributeTypeAndValue: {:#}",
                e
            );
            None
        }
    }
}

impl UserInfo {
    fn parse_from_x509(x509: &x509_parser::certificate::X509Certificate<'_>) -> Self {
        let subj = &x509.tbs_certificate.subject;
        let mut username = None;
        let mut groups = Vec::new();
        for cn in subj.iter_common_name() {
            let name = match parse_aval(cn) {
                Some(n) => n,
                None => continue,
            };
            if let Some(prev) = username.replace(name) {
                tracing::warn!(
                    ignored = prev.as_str(),
                    "certificate specifies more than one subject CN field, ignoring previous field"
                );
            }
        }
        for ou in subj.iter_organizational_unit() {
            let group = match parse_aval(ou) {
                Some(g) => g,
                None => continue,
            };
            groups.push(group);
        }
        UserInfo {
            username: username.unwrap_or_else(|| "system:missing".to_string()),
            groups,
        }
    }

    async fn auth_using_tls(request: &Request<'_>) -> rocket::request::Outcome<Self, Error> {
        ClientTls::from_request(request)
            .await
            .map_failure(|f| match f.1 {})
            .and_then(|c| {
                x509_parser::parse_x509_certificate(&c.end_entity.data())
                    .map(|(_, x509)| UserInfo::parse_from_x509(&x509))
                    .into_outcome(Status::InternalServerError)
            })
            .map_failure(|(status, err)| (status, err.into()))
    }

    async fn auth_using_request_header(
        request: &Request<'_>,
        cfg: &RequestHeaderAuthentifierConfig,
    ) -> rocket::request::Outcome<Self, Infallible> {
        let headers = request.headers();
        let username = match headers.get_one(&cfg.username) {
            Some(name) => name.to_string(),
            None => return rocket::request::Outcome::Forward(()),
        };
        let mut groups = Vec::new();
        for g in headers.get(&cfg.group) {
            groups.push(g.to_string());
        }

        rocket::request::Outcome::Success(UserInfo { username, groups })
    }
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for UserInfo {
    type Error = Error;

    async fn from_request(
        request: &'r rocket::Request<'_>,
    ) -> rocket::request::Outcome<Self, Self::Error> {
        let cfg: Option<&AuthentifierConfig> = request.rocket().state();

        let tls_identity = UserInfo::auth_using_tls(request).await;

        if !tls_identity.is_forward() {
            return tls_identity;
        }

        let reqheader_cfg = cfg.and_then(|c| c.request_header.as_ref());

        let reqheader_identity = match reqheader_cfg {
            Some(cfg) => UserInfo::auth_using_request_header(request, &cfg).await,
            None => rocket::request::Outcome::Forward(()),
        };

        if !reqheader_identity.is_forward() {
            return reqheader_identity.map_failure(|f| match f.1 {});
        }
        rocket::request::Outcome::Forward(())
    }
}
