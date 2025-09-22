use crate::db::models::{UserActiveModel, UserColumn, UserEntity, UserModel};
use anyhow::Result;
use sea_orm::DeleteResult;
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};

pub struct UserRepository<'a> {
    db: &'a DatabaseConnection,
}

impl<'a> UserRepository<'a> {
    pub fn new(db: &'a DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn find_by_id(&self, id: i32) -> Result<Option<UserModel>> {
        Ok(UserEntity::find_by_id(id).one(self.db).await?)
    }

    pub async fn find_by_username(&self, username: &str) -> Result<Option<UserModel>> {
        Ok(UserEntity::find()
            .filter(UserColumn::Username.eq(username))
            .one(self.db)
            .await?)
    }

    pub async fn find_by_email(&self, email: &str) -> Result<Option<UserModel>> {
        Ok(UserEntity::find()
            .filter(UserColumn::Email.eq(email))
            .one(self.db)
            .await?)
    }

    pub async fn create(&self, model: UserActiveModel) -> Result<UserModel> {
        Ok(model.insert(self.db).await?)
    }

    pub async fn delete(&self, id: i32) -> Result<DeleteResult> {
        Ok(UserEntity::delete_by_id(id).exec(self.db).await?)
    }
}
