use axum::{extract::FromRequestParts, http::StatusCode};
use error::NotFound;
use futures::FutureExt;
use std::convert::Infallible;
use std::{boxed::Box, future::Future, pin::Pin};
pub struct Token(pub Result<String, NotFound>);

pub struct Theme(pub theme::Theme);

impl<S> FromRequestParts<S> for Token
where
    S: Send,
{
    type Rejection = Infallible;
    fn from_request_parts<'a, 'b, 'c>(
        parts: &'a mut axum::http::request::Parts,
        _state: &'b S,
    ) -> Pin<Box<dyn Future<Output = Result<Self, Self::Rejection>> + Send + 'c>>
    where
        'a: 'c,
        'b: 'c,
    {
        async {
            let cookies = parts.headers.get(axum::http::header::COOKIE);
            if let Some(Ok(tmp)) =
                cookies.map(|e| e.to_str().map(|x| x.split(';').collect::<Vec<_>>()))
            {
                for i in tmp {
                    let cookie: Vec<_> = i.split("=").collect();
                    if let (Some(Ok(cookie::Cookie::TOKEN)), Some(value)) = (
                        cookie.first().map(|x| cookie::Cookie::try_from(*x)),
                        cookie.get(1),
                    ) {
                        return Ok(Self(Ok(value.to_string())));
                    }
                }
            }
            Ok(Self(Err(NotFound {
                key: cookie::Cookie::TOKEN.to_string(),
            })))
        }
        .boxed()
    }
}

impl<S> FromRequestParts<S> for Theme
where
    S: Send,
{
    type Rejection = Infallible;

    fn from_request_parts<'a, 'b, 'c>(
        parts: &'a mut axum::http::request::Parts,
        _state: &'b S,
    ) -> Pin<Box<dyn Future<Output = Result<Self, Self::Rejection>> + Send + 'c>>
    where
        'a: 'c,
        'b: 'c,
    {
        async {
            if let Some(e) = parts.headers.get(axum::http::header::COOKIE) {
                if let Ok(key_value) = e.to_str().map(|x| x.split(';').collect::<Vec<_>>()) {
                    for i in key_value {
                        let tmp: Vec<_> = i.split('=').collect();
                        if let (Some(Ok(self::cookie::Cookie::THEME)), Some(value)) = (
                            tmp.first().map(|x| self::cookie::Cookie::try_from(*x)),
                            tmp.get(1),
                        ) {
                            return Ok(Self(match self::theme::Theme::try_from(*value) {
                                Ok(e) => e,
                                _ => theme::Theme::Light,
                            }));
                        }
                    }
                }
            }
            Ok(Theme(theme::Theme::Light))
        }
        .boxed()
    }
}

pub mod cookie {
    #[derive(Debug, PartialEq)]
    pub enum Cookie {
        TOKEN,
        THEME,
    }

    impl std::fmt::Display for Cookie {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Self::TOKEN => write!(f, "jwt"),
                Self::THEME => write!(f, "theme"),
            }
        }
    }

    impl TryFrom<&str> for Cookie {
        type Error = super::error::ParseError;
        fn try_from(value: &str) -> Result<Self, Self::Error> {
            match value {
                "jwt" => Ok(Self::TOKEN),
                "theme" => Ok(Self::THEME),
                _ => Err(super::error::ParseError),
            }
        }
    }
}

pub mod theme {

    #[derive(Debug, PartialEq)]
    pub enum Theme {
        Dark,
        Light,
    }

    impl std::fmt::Display for Theme {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            use Theme::*;

            match self {
                Dark => write!(f, "dark"),
                Light => write!(f, "light"),
            }
        }
    }

    impl TryFrom<&str> for Theme {
        type Error = super::error::ParseError;
        fn try_from(value: &str) -> Result<Self, Self::Error> {
            match value {
                "dark" => Ok(Self::Dark),
                "light" => Ok(Self::Light),
                _ => Err(super::error::ParseError),
            }
        }
    }
}

pub mod error {
    use super::StatusCode;
    use axum::response::IntoResponse;

    #[derive(Debug)]
    pub struct NotFound {
        pub key: String,
    }

    impl IntoResponse for NotFound {
        fn into_response(self) -> axum::response::Response {
            (StatusCode::NOT_FOUND, format!("{} not found", self.key)).into_response()
        }
    }

    #[derive(Debug)]
    pub struct ParseError;
}

pub mod authentication {
    use bcrypt::{hash, verify, DEFAULT_COST};
    use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
    use serde::{de::DeserializeOwned, Serialize};
    use std::sync::LazyLock;

    static ALGORITHM_JWT: LazyLock<Algorithm> = LazyLock::new(|| Algorithm::HS256);

    pub trait Claim: std::fmt::Debug {}

    pub fn verify_passwd<T: AsRef<[u8]>>(pass: T, pass_db: &str) -> bool {
        verify(pass.as_ref(), pass_db).unwrap_or(false)
    }

    pub fn encrypt<T: AsRef<[u8]>>(pass: T) -> Result<String, error::Error> {
        Ok(hash(pass.as_ref(), DEFAULT_COST)?)
    }

    pub fn create_token<T>(claim: T) -> Result<String, error::Error>
    where
        T: Serialize + Claim,
    {
        let secret = std::env::var("SECRET_KEY")?;

        Ok(encode(
            &Header::new(*ALGORITHM_JWT),
            &claim,
            &EncodingKey::from_secret(secret.as_ref()),
        )?)
    }

    pub fn verify_token<T, B: AsRef<str>>(token: B) -> Result<T, error::Error>
    where
        T: DeserializeOwned + Claim,
    {
        let secret = std::env::var("SECRET_KEY")?;

        match decode(
            token.as_ref(),
            &DecodingKey::from_secret(secret.as_ref()),
            &Validation::new(*ALGORITHM_JWT),
        ) {
            Ok(e) => Ok(e.claims),
            Err(e) => Err(e.into()),
        }
    }

    pub mod error {
        #[derive(Debug)]
        pub enum Error {
            Encrypt,
            EncodeToken,
            SecretKey,
        }

        impl std::fmt::Display for Error {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                match self {
                    Error::Encrypt => write!(f, "Encrypt Error"),
                    Error::EncodeToken => write!(f, "Encode Token Error"),
                    Error::SecretKey => write!(f, "Secret key not found"),
                }
            }
        }

        impl std::error::Error for Error {}

        impl From<std::env::VarError> for Error {
            fn from(_value: std::env::VarError) -> Self {
                Self::SecretKey
            }
        }

        impl From<jsonwebtoken::errors::Error> for Error {
            fn from(_value: jsonwebtoken::errors::Error) -> Self {
                Self::EncodeToken
            }
        }

        impl From<bcrypt::BcryptError> for Error {
            fn from(_value: bcrypt::BcryptError) -> Self {
                Self::Encrypt
            }
        }
    }
}

#[allow(dead_code)]
pub mod response_error {
    use axum::{
        http::StatusCode,
        response::{IntoResponse, Response},
    };
    use serde::{Deserialize, Serialize};
    use time::{OffsetDateTime, UtcOffset};

    #[derive(Debug, Serialize, Deserialize)]
    pub struct ResponseError {
        #[serde(skip_serializing_if = "Option::is_none")]
        r#type: Option<String>,

        #[serde(skip_serializing_if = "Option::is_none")]
        title: Option<String>,

        #[serde(skip_serializing_if = "Option::is_none")]
        status: Option<u16>,

        #[serde(skip_serializing_if = "Option::is_none")]
        detail: Option<String>,

        #[serde(skip_serializing_if = "Option::is_none")]
        instance: Option<String>,

        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(with = "time::serde::rfc3339::option")]
        timestamp: Option<OffsetDateTime>,
    }

    impl ResponseError {
        pub fn new(
            r#type: String,
            title: String,
            status: StatusCode,
            detail: String,
            instance: String,
            offset: Option<UtcOffset>,
        ) -> Self {
            Self {
                r#type: Some(r#type),
                title: Some(title),
                status: Some(status.as_u16()),
                detail: Some(detail),
                instance: Some(instance),
                timestamp: Some(
                    OffsetDateTime::now_utc().to_offset(offset.unwrap_or(UtcOffset::UTC)),
                ),
            }
        }

        pub fn builder() -> Builder {
            Builder::default()
        }

        pub(self) fn create(
            Builder {
                r#type,
                title,
                status,
                detail,
                instance,
                offset,
            }: Builder,
        ) -> ResponseError {
            Self {
                r#type,
                title,
                status: status.or(Some(400)),
                detail,
                instance,
                timestamp: Some(
                    OffsetDateTime::now_utc().to_offset(offset.unwrap_or(UtcOffset::UTC)),
                ),
            }
        }
    }

    impl From<Builder> for ResponseError {
        fn from(value: Builder) -> Self {
            ResponseError::create(value)
        }
    }

    impl IntoResponse for ResponseError {
        fn into_response(self) -> axum::response::Response {
            Response::builder()
                .header(axum::http::header::CONTENT_TYPE, "application/problem+json")
                .status(StatusCode::from_u16(self.status.unwrap()).unwrap())
                .body(serde_json::json!(self).to_string())
                .unwrap_or_default()
                .into_response()
        }
    }

    #[derive(Debug, Default)]
    pub struct Builder {
        r#type: Option<String>,
        title: Option<String>,
        status: Option<u16>,
        detail: Option<String>,
        instance: Option<String>,
        offset: Option<UtcOffset>,
    }

    impl Builder {
        pub fn r#type(mut self, r#type: String) -> Self {
            self.r#type = Some(r#type);
            self
        }

        pub fn status(mut self, status: StatusCode) -> Self {
            self.status = Some(status.as_u16());
            self
        }

        pub fn title(mut self, title: String) -> Self {
            self.title = Some(title);
            self
        }

        pub fn detail(mut self, detail: String) -> Self {
            self.detail = Some(detail);
            self
        }

        pub fn instance(mut self, instance: String) -> Self {
            self.instance = Some(instance);
            self
        }

        pub fn offset(mut self, offset: time::UtcOffset) -> Self {
            self.offset = Some(offset);
            self
        }

        pub fn offset_hms(mut self, (hours, minutes, seconds): (i8, i8, i8)) -> Self {
            self.offset = UtcOffset::from_hms(hours, minutes, seconds).ok();
            self
        }

        pub fn build(self) -> ResponseError {
            ResponseError::create(self)
        }
    }

    impl From<ResponseError> for Builder {
        fn from(value: ResponseError) -> Self {
            let ResponseError {
                r#type,
                title,
                status,
                detail,
                instance,
                timestamp,
            } = value;
            Builder {
                r#type,
                title,
                status,
                detail,
                instance,
                offset: timestamp.map(|x| x.offset()),
            }
        }
    }
}

#[allow(dead_code)]
pub mod type_net {

    pub mod host_count {
        use ipnet::IpNet;
        use serde::{Deserialize, Serialize};

        #[derive(Debug, PartialEq)]
        pub enum Type {
            Limited,
            Unlimited,
        }

        impl std::fmt::Display for Type {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                match self {
                    Self::Limited => write!(f, "Limited"),
                    Self::Unlimited => write!(f, "Unlimited"),
                }
            }
        }

        pub struct Prefix{
            host_part: u8,
            network_part: u8,
        }

        impl Prefix {

            const MAX: u8 = 128;

            pub fn part_host(&self) -> u8 {
                self.host_part
            }
            pub fn set(&mut self, network: &IpNet) {
                *self = Prefix::from(network);
            }
            pub fn set_from_prefix(&mut self, prefix: &Prefix) {
                *self = Self{..*prefix};
            }
        }
        #[derive(Debug)]
        pub struct InvalidPrefix;

        impl PartialEq for Prefix {
            fn eq(&self, other: &Self) -> bool {
                self.network_part == other.network_part
            }
        }

        impl PartialEq<u8> for Prefix {
            fn eq(&self, other: &u8) -> bool {
                *other == self.network_part
            }
        }

        impl PartialOrd for Prefix {
            fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
                Some(self.cmp(other))
            }
        }

        impl PartialOrd<u8> for Prefix {
            fn partial_cmp(&self, other: &u8) -> Option<std::cmp::Ordering> {
                Some(self.cmp(other))
            }
        }

        impl From<&IpNet> for Prefix {
            fn from(value: &IpNet) -> Self {
                Self{
                    host_part: value.max_prefix_len() - value.prefix_len(),
                    network_part: value.prefix_len()
                }
            }
        }

        impl std::ops::Deref for Prefix {
            type Target = u8;
            fn deref(&self) -> &Self::Target {
                &self.network_part
            }
        }

        #[derive(Deserialize, Serialize, Debug, Clone)]
        #[serde(transparent)]
        pub struct HostCount(u32);

        impl HostCount {
            pub const MAX: u32 = u32::MAX;

            pub fn new(prefix: Prefix) -> Self {
                if prefix > 32 {
                    Self(Self::MAX)
                } else {
                    Self(2u32.pow(prefix.part_host().into()) - 2)
                }
            }

            pub fn unlimited(&self) -> bool {
                self.0 == Self::MAX
            }

            pub fn type_limit(&self, prefix: Prefix) -> Type {
                if prefix > 32 {
                    Type::Unlimited
                } else {
                    Type::Limited
                }
            }

            pub fn add<T: Into<u32>>(&mut self, rhs: T) -> Result<(), CountOfRange> {
                self.0 = self
                    .0
                    .checked_add(T::into(rhs))
                    .ok_or(CountOfRange)?;
                Ok(())
            }

            pub fn sub<T: Into<u32>>(&mut self, rhs: T) -> Result<(), CountOfRange> {
                self.0 = self
                    .0
                    .checked_sub(T::into(rhs))
                    .ok_or(CountOfRange)?;
                Ok(())
            }
        }

        impl From<u32> for HostCount {
            fn from(value: u32) -> Self {
                Self(value)
            }
        }

        impl std::ops::Deref for HostCount {
            type Target = u32;
            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }

        #[derive(Debug)]
        pub struct CountOfRange;

        #[cfg(test)]
        mod test {
            use crate::type_net::host_count::HostCount;

            use super::Prefix;
            use ipnet::IpNet; //dev-dependencies
            #[test]
            fn test_prefix_instance_exit() {
                let ipnet: IpNet = "172.30.0.30/24".parse().unwrap();
                let pref = Prefix::from(&ipnet);
                
                let ipnet: IpNet = "172.30.0.30/16".parse().unwrap();
                let pref_2 = Prefix::from(&ipnet);
                assert_eq!(24,*pref);
                assert_eq!(16, *pref_2);
            }

            #[test]
            fn test_prefix_instance_fail() {
                let ipnet: IpNet = "172.30.0.30/25".parse().unwrap();
                let pref = Prefix::from(&ipnet);
                
                let ipnet: IpNet = "172.30.0.30/13".parse().unwrap();
                let pref_2 = Prefix::from(&ipnet);
                assert_ne!(16,*pref);
                assert_ne!(24, *pref_2);
            }

            #[test]
            fn test_prefix_partial_eq_with_prefix() {
                let ipnet: IpNet = "172.30.0.30/25".parse().unwrap();
                let pref = Prefix::from(&ipnet);
                
                let ipnet: IpNet = "172.30.0.30/25".parse().unwrap();
                let pref_2 = Prefix::from(&ipnet);
                assert!(pref_2 == pref);
            }

            #[test]
            fn test_prefix_partial_eq_with_integer() {                
                let ipnet: IpNet = "172.30.0.30/25".parse().unwrap();
                let pref_2 = Prefix::from(&ipnet);
                assert!(pref_2 == 25);
            }

            #[test]
            fn test_prefix_partial_partial_ord_with_prefix() {
                let ipnet: IpNet = "172.30.0.30/24".parse().unwrap();
                let pref = Prefix::from(&ipnet);

                let ipnet: IpNet = "172.30.0.30/25".parse().unwrap();
                let pref_2 = Prefix::from(&ipnet);
                assert_eq!(pref_2 > pref, true);
                assert_eq!(pref_2 < pref, false);
                assert_ne!(pref_2 < pref, true);
                assert_ne!(pref_2 > pref, false);
                assert!(pref_2 > pref);
            }

            #[test]
            fn test_prefix_partial_partial_ord_with_integer() {
                let ipnet: IpNet = "172.30.0.30/24".parse().unwrap();
                let pref = Prefix::from(&ipnet);
                assert_eq!(pref > 10, true);
                assert_eq!(pref < 10, false);
                assert!(pref > 10);
            }

            #[test]
            fn host_counter_instance_from_prefix() {
                let pref = HostCount::new(Prefix::from(&"172.30.0.0/24".parse::<IpNet>().unwrap()));
                assert_eq!(*pref, 254);
            }
            #[test]
            fn host_counter_instance_from_u32() {
                let pref:HostCount = 10.into();
                assert_eq!(*pref, 10);
                assert_ne!(15, *pref);
            }

            #[test]
            fn host_counter_subtract_ok() {
                let mut pref:HostCount = 10.into();
                assert!(pref.sub(9u8).is_ok());
                assert!(pref.sub(1u8).is_ok());
            }

            #[test]
            fn host_counter_subtract_err() {
                let mut pref:HostCount = 10.into();
                assert!(pref.sub(1u8).is_ok());
                assert!(pref.sub(10u8).is_err());
            }
            #[test]
            fn host_counter_addition_ok() {
                let mut pref:HostCount = 10.into();
                assert!(pref.add(9u8).is_ok());
                assert!(pref.add(1u8).is_ok());
            }

            #[test]
            fn host_counter_addition_err() {
                let mut pref:HostCount = 10.into();
                assert!(pref.add(HostCount::MAX).is_err());
            }
        }
    }

    pub mod vlan {
        use serde::{de::Visitor, Deserialize, Serialize};

        pub struct Vlan(u16);

        impl Vlan {
            pub const MAX: u16 = 4095;

            pub fn vlan_id(id: u16) -> Result<Self, OutOfRange> {
                Ok(Vlan(Self::vlidate(id)?))
            }

            pub fn set_vlan(&mut self, id: u16) -> Result<(), OutOfRange> {
                self.0 = Self::vlidate(id)?;
                Ok(())
            }

            fn vlidate(id: u16) -> Result<u16, OutOfRange> {
                if id > Self::MAX {
                    Err(OutOfRange)
                } else {
                    Ok(id)
                }
            }
        }

        impl std::ops::Deref for Vlan {
            type Target = u16;
            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }

        #[derive(Debug)]
        pub struct OutOfRange;

        impl std::fmt::Display for OutOfRange {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "Out of range")
            }
        }
        impl std::error::Error for OutOfRange {}

        impl Serialize for Vlan {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                serializer.serialize_u16(**self)
            }
        }

        struct VlanVisitor;

        impl<'de> Deserialize<'de> for Vlan {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                deserializer.deserialize_any(VlanVisitor)
            }
        }

        impl<'de> Visitor<'de> for VlanVisitor {
            type Value = Vlan;
            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("Vlan id expected")
            }
            fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Self::visit_str(self, &v)
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                match v.parse::<u16>().map(Vlan::vlan_id) {
                    Ok(Ok(e)) => Ok(e),
                    _ => Err(E::custom(OutOfRange.to_string())),
                }
            }

            fn visit_u8<E>(self, v: u8) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Vlan::vlan_id(v as u16).map_err(|_| E::custom(OutOfRange.to_string()))
            }
            fn visit_u16<E>(self, v: u16) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                match Vlan::vlan_id(v) {
                    Ok(e) => Ok(e),
                    _ => Err(E::custom(OutOfRange.to_string())),
                }
            }

            fn visit_u32<E>(self, v: u32) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                if v > Vlan::MAX as u32 {
                    Err(E::custom(OutOfRange.to_string()))
                } else {
                    Ok(Vlan::vlan_id(v as u16).unwrap())
                }
            }

            fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                if v > Vlan::MAX as u64 {
                    Err(E::custom(OutOfRange.to_string()))
                } else {
                    Ok(Vlan::vlan_id(v as u16).unwrap())
                }
            }

            fn visit_u128<E>(self, v: u128) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                if v > Vlan::MAX as u128 {
                    Err(E::custom(OutOfRange.to_string()))
                } else {
                    Ok(Vlan::vlan_id(v as u16).unwrap())
                }
            }

            fn visit_i8<E>(self, v: i8) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                if v < 0 {
                    Err(E::custom(OutOfRange.to_string()))
                } else {
                    Ok(Vlan::vlan_id(v as u16).unwrap())
                }
            }

            fn visit_i16<E>(self, v: i16) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                if v < 0 {
                    Err(E::custom(OutOfRange.to_string()))
                } else {
                    Ok(Vlan::vlan_id(v as u16).unwrap())
                }
            }

            fn visit_i32<E>(self, v: i32) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                if v < 0 || v > Vlan::MAX as i32 {
                    Err(E::custom(OutOfRange.to_string()))
                } else {
                    Ok(Vlan::vlan_id(v as u16).unwrap())
                }
            }

            fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                if v < 0 || v > Vlan::MAX as i64 {
                    Err(E::custom(OutOfRange.to_string()))
                } else {
                    Ok(Vlan::vlan_id(v as u16).unwrap())
                }
            }

            fn visit_i128<E>(self, v: i128) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                if v < 0 || v > Vlan::MAX as i128 {
                    Err(E::custom(OutOfRange.to_string()))
                } else {
                    Ok(Vlan::vlan_id(v as u16).unwrap())
                }
            }
        }
    }
}

pub mod ipam_services {
    use std::net::IpAddr;

    use axum::{http::{Response, StatusCode}, response::IntoResponse};
    use ipnet::IpNet;

    #[derive(Debug)]
    pub struct SubnettingError(pub String);

    impl std::fmt::Display for SubnettingError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "Subnneting can't create")
        }
    }

    impl std::error::Error for SubnettingError {}

    pub async fn subnetting(ipnet: IpNet, prefix: u8) -> Result<Vec<IpNet>, SubnettingError> {
        let ip = ipnet.netmask();
        let mut resp = Vec::new();
        let sub = 2u32.pow((ipnet.prefix_len() - prefix) as u32);
        if sub == 0 {
            return Err(SubnettingError(format!("Subnet {}/{} is not valid for the network {}",ip, sub, prefix)));
        }
        let ip = ip.to_string();
        for _ in 0..sub {
            resp.push(format!("{}/{}", ip, prefix).parse().map_err(|x: ipnet::AddrParseError| SubnettingError(x.to_string()))?);
        }
        Ok(resp)
    }

    pub async fn ping(ip: IpAddr, timeout_ms: u64) -> Ping {
        let ip = ip.to_string();
        let duration = std::time::Duration::from_millis(timeout_ms).as_secs_f32().to_string();
        let ping = tokio::process::Command::new("ping")
            .args(["-W", &duration, "-c", "1", &ip])
            .output()
            .await; 

        match ping {
            Ok(e) if e.status.code().unwrap_or(1) == 0 => Ping::Pong,
            _ => Ping::Fail,
        }
    }

    #[derive(Debug, PartialEq, PartialOrd, serde::Serialize)]
    pub enum Ping {
        Pong,
        Fail,
    }
    impl std::fmt::Display for Ping {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Self::Pong => write!(f, "Pong"),
                Self::Fail => write!(f, "Fail"),
            }
        }
    }
    impl IntoResponse for Ping {
        fn into_response(self) -> axum::response::Response {
            Response::builder()
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .status(StatusCode::OK)
                .body(serde_json::json!({
                    "status": 200,
                    "ping": self.to_string()
                }).to_string())
                .unwrap_or_default()
                .into_response()
        }
    }

    #[cfg(test)]
    mod test {
        use super::*;
        use std::sync::LazyLock;
        use tokio::runtime::Runtime;

        static RUNTIME: LazyLock<Runtime> = std::sync::LazyLock::new(|| {Runtime::new().unwrap()});
        #[test]
        fn ping_test_pong() {
            let resp = RUNTIME.block_on(async {ping("192.168.0.1".parse().unwrap(),1).await });
            assert_eq!(Ping::Pong, resp);
        }

        #[test]
        fn ping_test_fail() {
            let resp = RUNTIME.block_on(async {ping("192.168.1.50".parse().unwrap(), 1).await });
            assert_eq!(Ping::Fail, resp);
        }
    }
}