use super::*;

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct User {
    pub id: uuid::Uuid,
    pub username: String,
    pub password: String,
    pub role: Role,
}

#[derive(Deserialize, Serialize, sqlx::Type, Debug, Clone, PartialEq)]
pub enum Role {
    Admin,
    Guest,
    Operator,
}