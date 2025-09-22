use proto::mpc::Chain as ProtoChain;
use sea_orm::{
    entity::prelude::*,
    sqlx::types::chrono::{DateTime, Utc},
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::None)")]
pub enum Chain {
    #[sea_orm(string_value = "ethereum")]
    Ethereum,
    #[sea_orm(string_value = "bitcoin")]
    Bitcoin,
}

impl From<Chain> for i32 {
    fn from(val: Chain) -> Self {
        match val {
            Chain::Ethereum => ProtoChain::Ethereum as i32,
            Chain::Bitcoin => ProtoChain::Bitcoin as i32,
        }
    }
}

#[derive(Debug, Clone, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "tbl_wallets")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub user_id: i32,
    pub name: String,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
    pub chain: Chain,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::user::Entity",
        from = "Column::UserId",
        to = "super::user::Column::Id"
    )]
    User,
}

impl Related<super::user::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::User.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
